use super::node::BeaconBlock;
use super::{blocks, Slot};
use crate::units::GweiNewtype;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgRow, PgExecutor, Row};

// accumulate each block#deposits' inner defined amount value together
// and finally return the accumulated amount values in the unit of GweiNewtype (i64)
pub fn get_deposit_sum_from_block(block: &BeaconBlock) -> GweiNewtype {
    block
        .deposits()
        .iter()
        .fold(GweiNewtype(0), |sum, deposit| sum + deposit.amount)
}

/// Computes the aggregated sum of deposit amounts for a given beacon block.
///
/// - If the block is the genesis block, returns `0` since it has no associated deposits.
/// - Otherwise, retrieves the parent block's deposit sum and adds the current block's deposits.
///
/// # Arguments
/// * `executor` - A database executor for querying deposit data.
/// * `block` - The beacon block for which the deposit sum needs to be computed.
///
/// # Returns
/// A `GweiNewtype` representing the total deposit sum aggregated up to the given block.
pub async fn get_deposit_sum_aggregated(
    executor: impl PgExecutor<'_>,
    block: &BeaconBlock,
) -> GweiNewtype {
    let parent_deposit_sum_aggregated = if block.slot == Slot::GENESIS {
        GweiNewtype(0)
    } else {
        blocks::get_deposit_sum_from_block_root(executor, &block.parent_root)
            .await
    };
    // block's parent deposit sum value + current block's all deposit amount value together
    parent_deposit_sum_aggregated + get_deposit_sum_from_block(block)
}
