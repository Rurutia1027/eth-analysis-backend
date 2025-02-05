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

// get each deposit's amount fields' aggregated sum value as return value
// by providing the block
// if given beacon block's slot is GENESIS it is the root block, it's associated deposit items is none, so the return value is 0
// otherwise, invoke request to beacon_blocks table and query all records with record's block_root the same as given block's parent_root value
// then fetch this record's
// query(block) -> beacon_blocks -> got current query block's parent block hash value as the return record
// then get the parent block record's  deposit_sum_aggregated this is current block's all deposit amount sum value as parent_deposit_sum_aggregated
// then, traverse current block's all deposit's amount value as the return result
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
