use super::node::{BeaconBlock, Withdrawal};
use super::{blocks, Slot, SHAPELLA_SLOT};
use crate::units::GweiNewtype;
use serde::{Deserialize, Serialize};
use sqlx::PgExecutor;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BeaconWithdrawalsSum {
    pub slot: Slot,
    pub withdrawals_sum: GweiNewtype,
}

pub fn get_withdrawal_sum_from_block(block: &BeaconBlock) -> GweiNewtype {
    match block.withdrawals() {
        Some(withdrawals) => withdrawals
            .iter()
            .fold(GweiNewtype(0), |sum, withdrawal| sum + withdrawal.amount),
        None => GweiNewtype(0),
    }
}

pub async fn get_withdrawal_sum_aggregated(
    executor: impl PgExecutor<'_>,
    block: &BeaconBlock,
) -> GweiNewtype {
    let parent_withdrawal_sum_aggregated = if block.slot < *SHAPELLA_SLOT {
        GweiNewtype(0)
    } else {
        blocks::get_withdrawal_sum_from_block_root(executor, &block.parent_root)
            .await
    };

    parent_withdrawal_sum_aggregated + get_withdrawal_sum_from_block(block)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beacon_chain::{node::Withdrawal, BeaconBlockBuilder};

    #[test]
    fn zero_withdrawals_test() {
        let block = BeaconBlockBuilder::default().build();
        assert_eq!(get_withdrawal_sum_from_block(&block), GweiNewtype(0));
    }

    #[test]
    fn some_withdrawals_test() {
        let block = BeaconBlockBuilder::default()
            .block_hash("0xwithdrawals_test")
            .slot(*SHAPELLA_SLOT + 1)
            .withdrawals(vec![
                Withdrawal {
                    index: 0,
                    address: "0x000000".to_string(),
                    amount: GweiNewtype(1),
                },
                Withdrawal {
                    index: 1,
                    address: "0x000000".to_string(),
                    amount: GweiNewtype(2),
                },
            ])
            .build();
        assert_eq!(get_withdrawal_sum_from_block(&block), GweiNewtype(3));
    }
}
