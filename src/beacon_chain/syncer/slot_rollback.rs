use sqlx::{Acquire, PgConnection};
use tracing::debug;
use crate::beacon_chain::{balances, blocks, issuance, states, Slot};

// this function will delete multiple records from beacon tables,
// that the records locates by the given slot range [given_slot, ...)
pub async fn rollback_slots(
    executor: &mut PgConnection,
    greater_than_or_equal: Slot,
) -> anyhow::Result<()> {
    debug!("rolling back data based on slots locates in range of [{greater_than_or_equal}, ...]");
    let mut transaction = executor.begin().await?;
    // todo: update table eth_supply but we haven't implement this table's associated function yet, leave a todo here
    blocks::delete_blocks(&mut *transaction, greater_than_or_equal).await;
    issuance::delete_issuances(&mut *transaction, greater_than_or_equal).await;
    balances::delete_validator_sums(&mut *transaction, greater_than_or_equal)
        .await;
    states::delete_states(&mut *transaction, greater_than_or_equal).await;
    transaction.commit().await?;
    Ok(())
}

// this function will delete records from multiple beacon tables
// that the records in the beacon tables share the same slot value provided by the parameter
pub async fn rollback_slot(
    executor: &mut PgConnection,
    slot: Slot,
) -> anyhow::Result<()> {
    debug!("rolling back data from db tables based on the given slot {slot}");
    let mut transaction = executor.begin().await?;
    // todo: update table eth_supply but we haven't implement this table's associated function yet, leave a todo here
    // first - delete block record in beacon_blocks table that the block locates in the given slot period(12 s) on beacon chain
    blocks::delete_block(&mut *transaction, slot).await;

    // second - delete issuance records in beacon_issuance table
    issuance::delete_issuance(&mut *transaction, slot).await;

    // third - delete validator sum from beacon_validators_balance tabel
    balances::delete_validator_sum(&mut *transaction, slot).await;

    // last -- delete record from table beacon_states -- this should be the last delete, because the above table deletion all refers to
    // record in beacon_states
    states::delete_state(&mut *transaction, slot).await;
    transaction.commit().await?;
    Ok(())
}

