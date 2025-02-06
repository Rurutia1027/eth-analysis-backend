pub mod backfill;
mod effective_sums;

use super::node::{BeaconNode, BeaconNodeHttp, ValidatorBalance};
use super::{states::get_last_state, GweiInTime, Slot};
use crate::units::GweiNewtype;
use chrono::{Duration, DurationRound};
use serde::{Deserialize, Serialize};
use sqlx::{PgExecutor, PgPool};

// this function will iterate and accumulate all passed in ValidatorBalance#balance field
// value and return
pub fn sum_validator_balances(
    validator_balances: &[ValidatorBalance],
) -> GweiNewtype {
    // validator_balances is an array of instance ValidatorBalance
    // here we iterate each item of the instance of ValidatorBalance
    // and create an init value as 0
    // traver each item and accumulate each instance#balance value
    // finally return value in type of GweiNewtype(alais as i64)
    validator_balances
        .iter()
        .fold(GweiNewtype(0), |sum, validator_balance| {
            sum + validator_balance.balance
        })
}

// function implement insert timestamp, state_root value and balance value as gwei(i64)
// to table beacon_validators_balance
pub async fn store_validators_balance(
    pool: impl PgExecutor<'_>,
    state_root: &str, // state_root is the anchor that links 4 tables together
    slot: Slot,
    gwei: &GweiNewtype,
) {
    // converted GweiNewtype into i64
    let gwei: i64 = gwei.to_owned().into();

    sqlx::query!(
        "
        INSERT INTO
            beacon_validators_balance(timestamp, state_root, gwei)
        VALUES ($1, $2, $3)
        ",
        slot.date_time(),
        state_root,
        gwei
    )
    .execute(pool)
    .await
    .unwrap();
}

// function accumulate the last state's balance item's sum
// first, query db table beacon_states to fetch the latest state value
// then, take the latest state value send request to beacon api endpoint to fetch all the
//      balance data records that with the same state value --> result is an array of vector of ValidatorEnvelope items
// last, traverse each item in the vector and get each item's effective_balance() value aggregate the value and return
pub async fn get_last_effective_balance_sum(
    executor: impl PgExecutor<'_>,
    beacon_node: &BeaconNodeHttp,
) -> GweiNewtype {
    let last_state_root = get_last_state(executor)
        .await
        .expect("can not calculate a last effective balance with an empty beacon_states table")
        .state_root;

    beacon_node
        .get_validators_by_state(&last_state_root)
        .await
        .map(|validators| {
            validators.iter().fold(GweiNewtype(0), |sum, validator| {
                sum + validator.effective_balance()
            })
        })
        .unwrap()
}

// query gwei field from beacon_validators_balance table
// each returned record's timestamp should be distinct and timestamp should be located in today's timestamp scope.
pub async fn get_validator_balances_by_start_of_day(
    executor: impl PgExecutor<'_>,
) -> Vec<GweiInTime> {
    sqlx::query!(
        r#"
        SELECT
            DISTINCT ON (DATE_TRUNC('day', timestamp)) DATE_TRUNC('day', timestamp) AS "day_timestamp!",
            gwei
        FROM
            beacon_validators_balance
        ORDER BY
            DATE_TRUNC('day', timestamp)
        "#
    )
        .fetch_all(executor)
        .await
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    GweiInTime {
                        t: row.day_timestamp.duration_trunc(Duration::days(1)).unwrap().timestamp() as u64,
                        v: row.gwei,
                    }
                })
                .collect()
        }).unwrap()
}

// function deletes multiple records in beacon_validators_balance table
// that with each slot value >= given slot value
// this function should be triggered once the record in the beacon_states is deleted
pub async fn delete_validator_sums(
    executor: impl PgExecutor<'_>,
    greater_than_or_equal: Slot,
) {
    sqlx::query!(
        "
        DELETE FROM beacon_validators_balance
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

// function deletes multiple records in beacon_validators_balance table with the same given slot value
// however, slot value does not exist in table so we need to first
// query block_states table by given slot value
// then use the queried records' state_root values as a set
// all records in beacon_validators_balance table with the same state_root value should be removed from the table
pub async fn delete_validator_sum(executor: impl PgExecutor<'_>, slot: Slot) {
    sqlx::query!(
        "
        DELETE FROM beacon_validators_balance
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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BeaconBalancesSum {
    pub slot: Slot,
    pub balances_sum: GweiNewtype,
}

// query gwei field from table beacon_validators_balance#gwei
// field, by providing state_root value as query condition
// field value will be converted from i64 into GweiNewType
pub async fn get_balances_by_state_root(
    executor: impl PgExecutor<'_>,
    state_root: &str,
) -> Option<GweiNewtype> {
    sqlx::query!(
        "
        SELECT
            gwei
        FROM
            beacon_validators_balance
        WHERE
            beacon_validators_balance.state_root = $1
        ",
        state_root
    )
    .fetch_optional(executor)
    .await
    .unwrap()
    .map(|row| {
        let gwei_i64: i64 = row.gwei;
        let gwei: GweiNewtype = gwei_i64.into();
        gwei
    })
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Duration, DurationRound, TimeZone, Utc};
    use sqlx::Connection;

    use crate::{beacon_chain::states::store_state, db::db};

    use super::*;

    #[tokio::test]
    async fn timestamp_is_start_of_day_test() {
        let mut connection = db::tests::get_test_db_connection().await;
        let mut transaction = connection.begin().await.unwrap();

        store_state(&mut *transaction, "0xtest_balances", Slot(17999)).await;
        store_validators_balance(
            &mut *transaction,
            "0xtest_balances",
            Slot(17999),
            &GweiNewtype(100),
        )
        .await;

        let validator_balances_by_day =
            get_validator_balances_by_start_of_day(&mut *transaction).await;

        let unix_timestamp = validator_balances_by_day.first().unwrap().t;
        let datetime = Utc.timestamp_opt(unix_timestamp as i64, 0).unwrap();

        let start_of_day_datetime =
            datetime.duration_trunc(Duration::days(1)).unwrap();

        assert_eq!(datetime, start_of_day_datetime)
    }

    #[tokio::test]
    async fn delete_balance_test() {
        let mut connection = db::tests::get_test_db_connection().await;
        let mut transaction = connection.begin().await.unwrap();

        // insert to beacon_states
        store_state(&mut *transaction, "0xtest_balances", Slot(17999)).await;

        // insert to beacon_validators_balance
        store_validators_balance(
            &mut *transaction,
            "0xtest_balances",
            Slot(17999),
            &GweiNewtype(100),
        )
        .await;

        // query today's beacon_validators_balance records in vector
        let balances =
            get_validator_balances_by_start_of_day(&mut *transaction).await;

        // vector length should be 1
        assert_eq!(balances.len(), 1);

        // delete by given Slot(0) -> first query beacon_states get state_root value
        // then match state_root value from beacon_validators_balance table
        // finally delete the inserted record from beacon_validators_balance
        delete_validator_sums(&mut *transaction, Slot(0)).await;

        // since record already deleted, queried vector length should be 0
        let balances =
            get_validator_balances_by_start_of_day(&mut *transaction).await;
        assert_eq!(balances.len(), 0);
    }

    // #[tokio::test]
    async fn get_balances_by_state_root_test() {
        let mut connection = db::tests::get_test_db_connection().await;
        let mut transaction = connection.begin().await.unwrap();
        let test_id = "get_balances_by_state_root";
        let state_root = format!("0x{test_id}_state_root");

        // store_test_block() <--- this has not been implement yet
        store_validators_balance(
            &mut *transaction,
            &state_root,
            Slot(0),
            &GweiNewtype(100),
        )
        .await;
        let beacon_balances_sum =
            get_balances_by_state_root(&mut *transaction, &state_root)
                .await
                .unwrap();
        assert!(true);
        todo!("here test logic should be refined after we implement\
         store_block which responsible for inserting beacon blocks to beacon_blocks table");
        assert_eq!(GweiNewtype(100), beacon_balances_sum);
    }
}
