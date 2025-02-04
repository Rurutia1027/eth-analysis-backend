pub mod backfill;

use super::node::{BeaconNode, BeaconNodeHttp, ValidatorBalance};
use super::{states::get_last_state, Slot};
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
