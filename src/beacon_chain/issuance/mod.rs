use super::Slot;
use crate::beacon_chain::node::Withdrawal;
use crate::beacon_chain::states::get_last_state;
use crate::{db::db, units::GweiNewtype};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::join;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::types::PgInterval, PgExecutor, PgPool};
use thiserror::Error;
use tracing::{debug, info};
use tracing_subscriber::fmt::time;

// insert new records to table beacon_issuance(timestamp, state_root, gwei)
// which state_root is link to pk in table beacon_states
pub async fn store_issuance(
    executor: impl PgExecutor<'_>,
    state_root: &str,
    slot: Slot,
    gwei: &GweiNewtype,
) {
    let gwei: i64 = gwei.to_owned().into();
    sqlx::query!(
        "
            INSERT INTO beacon_issuance (timestamp, state_root, gwei)
            VALUES ($1, $2, $3)
        ",
        slot.date_time(),
        state_root,
        gwei
    )
    .execute(executor)
    .await
    .unwrap();
}

// calculate the value of issuance
// issuance = validator_balances_sum_gwei + withdrawal_sum_aggregated - deposit_sum_aggregated
pub fn calc_issuance(
    validator_balances_sum_gwei: &GweiNewtype,
    withdrawal_sum_aggregated: &GweiNewtype,
    deposit_sum_aggregated: &GweiNewtype,
) -> GweiNewtype {
    (*validator_balances_sum_gwei + *withdrawal_sum_aggregated)
        - *deposit_sum_aggregated
}

// get the latest(freshest) issuance gwei value from table beacon_issuance
// and converted the value into GweiNewType
pub async fn get_current_issuance(
    executor: impl PgExecutor<'_>,
) -> GweiNewtype {
    sqlx::query!(
        "
            SELECT
                gwei
            FROM
                beacon_issuance
            ORDER BY
                timestamp DESC
            LIMIT 1
        ",
    )
    .fetch_one(executor)
    .await
    .map(|row| GweiNewtype(row.gwei))
    .unwrap()
}

// delete multiple records in beacon_issuance which join to beacon_state's slot values is >= given slot value
// slot only exists in table beacon_states table, so we need first query matching records
// in table beacon_states by given slot value
// then create sets based on the queried records' state_root value as the STATE_ROOT_SET
// then query records from table beacon_issuance with state_root field value locates in the STATE_ROOT_SET
pub async fn delete_issuances(
    executor: impl PgExecutor<'_>,
    greater_than_or_equal: Slot,
) {
    sqlx::query!(
        "
            DELETE FROM beacon_issuance
            WHERE state_root IN (
                SELECT state_root FROM beacon_states
                WHERE slot >= $1
            )
        ",
        greater_than_or_equal.0
    )
    .execute(executor)
    .await
    .unwrap();
}

// delete records in beacon_issuance table by match with only one slot value
pub async fn delete_issuance(executor: impl PgExecutor<'_>, slot: Slot) {
    sqlx::query!(
        "
        DELETE FROM beacon_issuance
        WHERE state_root IN (
            SELECT state_root FROM beacon_states
            WHERE slot = $1
        )
        ",
        slot.0
    )
    .execute(executor)
    .await
    .unwrap();
}

pub async fn get_n_days_ago_issuance(
    executor: impl PgExecutor<'_>,
    n: i32,
) -> GweiNewtype {
    sqlx::query!(
        "
            WITH issuance_distances AS (
                SELECT
                    gwei,
                    timestamp ,
                    ABS (
                        EXTRACT (
                            epoch
                            FROM
                            (timestamp - (NOW() - $1::INTERVAL))
                        )
                    ) AS distance_seconds
                    FROM
                        beacon_issuance
                    ORDER BY
                        distance_seconds ASC
                    LIMIT 100
            )
            SELECT gwei
            FROM issuance_distances
            WHERE distance_seconds <= 172800
            LIMIT 1
        ",
        PgInterval {
            days: 0,
            microseconds: 0,
            months: 0,
        }
    )
    .fetch_one(executor)
    .await
    .map(|row| GweiNewtype(row.gwei))
    .unwrap()
}

#[derive(Error, Debug)]
pub enum IssuanceUnavailableError {
    #[error("Issuance unavailable for timestamp {0}")]
    Timestamp(DateTime<Utc>),
}

// here we define a series of beacon_issuances table operations
#[async_trait]
pub trait IssuanceStore {
    async fn current_issuance(&self) -> GweiNewtype;
    async fn n_days_ago_issuance(&self, n: i32) -> GweiNewtype;
    async fn issuance_at_timestamp(
        &self,
        timestamp: DateTime<Utc>,
    ) -> Result<GweiNewtype, IssuanceUnavailableError>;

    async fn issuance_from_time_frame(
        &self,
        // block: &ExecutionNodeBlock,
        // time_frame: &TimeFrame,
    ) -> Result<GweiNewtype, IssuanceUnavailableError>;
    async fn weekly_issuance(&self) -> GweiNewtype;
}

pub struct IssuanceStoragePostgres {
    db_pool: PgPool,
}

impl IssuanceStoragePostgres {
    pub fn new(pool: PgPool) -> Self {
        Self { db_pool: pool }
    }
}

#[async_trait]
impl IssuanceStore for IssuanceStoragePostgres {
    async fn current_issuance(&self) -> GweiNewtype {
        get_current_issuance(&self.db_pool).await
    }

    async fn n_days_ago_issuance(&self, n: i32) -> GweiNewtype {
        get_n_days_ago_issuance(&self.db_pool, n).await
    }

    async fn issuance_at_timestamp(
        &self,
        timestamp: DateTime<Utc>,
    ) -> Result<GweiNewtype, IssuanceUnavailableError> {
        sqlx::query!(
            "
                SELECT gwei AS \"gwei!\"
                FROM beacon_issuance
                WHERE timestamp >= $1
                ORDER BY timestamp ASC
                LIMIT 1
            ",
            timestamp
        )
        .fetch_optional(&self.db_pool)
        .await
        .unwrap()
        .map_or_else(
            || Err(IssuanceUnavailableError::Timestamp(timestamp)),
            |row| Ok(GweiNewtype(row.gwei)),
        )
    }

    // todo: missing params define in the scope of execution chain
    async fn issuance_from_time_frame(
        &self,
    ) -> Result<GweiNewtype, IssuanceUnavailableError> {
        Ok(GweiNewtype(0))
    }

    /// weekly issuance in Gwei
    async fn weekly_issuance(&self) -> GweiNewtype {
        let (d14_issuance, now_issuance) =
            join!(self.n_days_ago_issuance(14), self.current_issuance());

        GweiNewtype((now_issuance - d14_issuance).0 / 2)
    }
}

const SLOTS_PER_MINUTE: u64 = 5; // 60 / 12s = 5
const MINUTES_PER_HOUR: u64 = 60;

const HOURS_PER_DAY: u64 = 24;
const DAYS_PER_WEEK: u64 = 7;
const SLOTS_PER_WEEK: f64 =
    (SLOTS_PER_MINUTE * MINUTES_PER_HOUR * HOURS_PER_DAY * DAYS_PER_WEEK)
        as f64;

#[derive(Debug, Serialize)]
struct IssuanceEstimate {
    slot: Slot,
    timestamp: DateTime<Utc>,
    issuance_per_slot_gwei: f64,
}

/// Calculate the estimated issuance per flot in Gwei.
/// Fetches last week's total issuance and divides it by the number of slots per week.
/// Returns `None` if the issuance data is unavailable
async fn get_issuance_per_slot_estimate(
    issuance_store: &impl IssuanceStore,
) -> f64 {
    let last_week_issuance = issuance_store.weekly_issuance().await;
    last_week_issuance.0 as f64 / SLOTS_PER_WEEK
}

// this is also the main entry point of issuance estimate service
// and this main entry function will be invoked in update-issuance-estimate.ts (not implemented yet)
// also the calculated final result will be updated to the project cache store(not implemented yet)
pub async fn update_issuance_estimate() {
    info!("updating issuance estimate");
    // create db connection pool instance with max connection = 3, and pool name as 'update-issuance-estimate'
    let db_pool = db::get_db_pool("update-issuance-estimate", 3).await;
    let issuance_store = IssuanceStoragePostgres::new(db_pool.clone());

    // get how many issuances in gwei per slot
    let issuance_per_slot_gwei =
        get_issuance_per_slot_estimate(&issuance_store).await;
    debug!("issuance per slot estimate: {}", issuance_per_slot_gwei);

    // here we get the freshest/latest state_root from the beacon_states table
    let slot = get_last_state(&db_pool)
        .await
        .expect(
            "expect last state to exist in order to update issuance estimate",
        )
        .slot;

    // get the timestamp value of the beacon_states' latest record
    let timestamp = slot.date_time();

    // create instance of struct IssuanceEstimate value by passing the values of
    // slot value, latest beacon_state's ts, and the estimateed issuance per slot value in the unit of Gwei
    let issuance_estimate = IssuanceEstimate {
        slot,
        timestamp,
        issuance_per_slot_gwei,
    };

    // finally publish the aggregated value struct instance to cache to let frontend request to fetch
    // but cache we haven't implment yet , just add a todo!() and print the value for now is ok
    todo!("publish the calculated issuance estimate value to the cache");
    info!("updated issuance estimate")
}
