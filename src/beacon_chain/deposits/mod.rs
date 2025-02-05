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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BeaconDepositsSum {
    pub deposits_sum: GweiNewtype,
    pub slot: Slot,
}

pub async fn get_deposits_sum_by_state_root(
    executor: impl PgExecutor<'_>,
    state_root: &str,
) -> Result<GweiNewtype> {
    let deposit_sum_aggregated = sqlx::query(
        "
                SELECT
                    deposit_sum_aggregated
                FROM
                    beacon_blocks
                WHERE
                    state_root = $1
            ",
    )
    .bind(state_root)
    .map(|row: PgRow| row.get::<i64, _>("deposit_sum_aggregated").into())
    .fetch_one(executor)
    .await?;

    Ok(deposit_sum_aggregated)
}

#[cfg(test)]
mod tests {
    use sqlx::Acquire;

    use crate::{
        beacon_chain::{
            blocks::store_block, states::store_state, BeaconBlockBuilder,
            BeaconHeaderSignedEnvelopeBuilder,
        },
        db::db,
    };

    use super::*;

    #[tokio::test]
    async fn get_deposits_sum_by_state_root_test() {
        let mut connection = db::tests::get_test_db_connection().await;
        let mut transaction = connection.begin().await.unwrap();
        let test_id = "get_deposits_sum_by_state_root";
        let test_header =
            BeaconHeaderSignedEnvelopeBuilder::new(test_id, Slot(222)).build();
        let test_block = Into::<BeaconBlockBuilder>::into(&test_header).build();

        // first save record to beacon_states
        store_state(
            &mut *transaction,
            &test_header.state_root(),
            test_header.slot(),
        )
        .await;

        // then save record to beacon_blocks
        store_block(
            &mut *transaction,
            &test_block,
            &GweiNewtype(0),
            &GweiNewtype(1),
            &GweiNewtype(0),
            &GweiNewtype(1),
            &test_header,
        )
        .await;

        // sum = current block's all deposit#amount + current block's parent's block_states#deposit_aggreated_sum field value
        let deposits_sum = get_deposits_sum_by_state_root(
            &mut *transaction,
            &test_header.state_root(),
        ).await.unwrap();

        assert_eq!(GweiNewtype(1), deposits_sum)
    }
}
