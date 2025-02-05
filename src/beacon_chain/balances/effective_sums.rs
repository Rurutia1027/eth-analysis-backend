use super::{BeaconNode, Slot};
use crate::beacon_chain::node::StateRoot;
use crate::units::{GweiImprecise, GweiNewtype};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgExecutor;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EffectiveBalanceSum {
    /// this amount is larger than 9M ETH, so we lose precision when serialization.
    /// For now this prevision issue is ignored.
    pub sum: GweiImprecise,
    pub slot: Slot,
    pub timestamp: DateTime<Utc>,
}

impl EffectiveBalanceSum {
    pub fn new(slot: Slot, sum: GweiNewtype) -> Self {
        Self {
            sum: sum.into(),
            slot,
            timestamp: slot.date_time(),
        }
    }
}

// retrieve all active ValidatorEnvelope as vector from beacon api endpoint
// by the provided state_root value
// then accumulate all filtered active ValidatorEnvelope#effective_balance value
// and return the value in unit of GweiNewType
pub async fn get_effective_balance_sum(
    beacon_node: &impl BeaconNode,
    state_root: &StateRoot,
) -> GweiNewtype {
    beacon_node
        .get_validators_by_state(state_root)
        .await
        .unwrap()
        .iter()
        .filter(|item| item.is_active())
        // aggregate the effective balance of all validators in the state
        .fold(GweiNewtype(0), |sum, item| sum + item.effective_balance())
}

// store the accumulated sum value of effective_balance to beacon_states table's effective_balance_sum field
pub async fn store_effective_balance_sum(
    executor: impl PgExecutor<'_>,
    state_root: &str,
    sum: &GweiNewtype,
) {
    sqlx::query!(
        "
        UPDATE
            beacon_states
        SET
            effective_balance_sum = $1
        WHERE
            state_root = $2
        ",
        sum.0,
        state_root
    )
    .execute(executor)
    .await
    .unwrap();
}

#[cfg(test)]
mod tests {
    use anyhow::{anyhow, Result};
    use async_trait::async_trait;
    use sqlx::Acquire;
    use test_context::test_context;

    use super::*;
    use crate::beacon_chain::states::store_state;
    use crate::db::db;
    use crate::{
        beacon_chain::{
            self,
            node::{
                BeaconBlock, BeaconHeaderSignedEnvelope, BlockId,
                FinalityCheckpoint, StateRoot, Validator, ValidatorBalance,
                ValidatorEnvelope,
            },
        },
        db::db::tests::TestDb,
        units::GweiNewtype,
    };

    const SLOT_0_STATE_ROOT: &str = "0x_mock_slot_state_root";

    #[tokio::test]
    async fn test_get_effective_balance_sum() {
        let mock_beacon_node = MockBeaconNode {};
        let state_root = SLOT_0_STATE_ROOT.to_string();
        let expected_sum = GweiNewtype(64_000_000_000_000_000);

        let sum =
            get_effective_balance_sum(&mock_beacon_node, &state_root).await;
        assert_eq!(sum, expected_sum);
    }

    #[tokio::test]
    async fn test_store_effective_balance_sum() {
        let mut connection = db::tests::get_test_db_connection().await;
        let mut transaction = connection.begin().await.unwrap();
        let state_root = SLOT_0_STATE_ROOT;
        let sum = GweiNewtype(9500000);

        // save record of beacon_states with its inner field effective_balance_sum as empty
        store_state(&mut *transaction, state_root, Slot(1000)).await;
        // append the effective_balance_sum field value to the record that is inserted
        store_effective_balance_sum(&mut *transaction, state_root, &sum).await;

        // query value of effective_balance_sum value by the state_root value
        // and fetch the record's effective_balance_sum
        let stored_sum: i64 = sqlx::query!(
            "
            SELECT effective_balance_sum
            FROM beacon_states
            WHERE state_root = $1
            ",
            state_root
        )
        .fetch_one(&mut *transaction)
        .await
        .unwrap()
        .effective_balance_sum
        .unwrap();

        // value should match the inserted sum value: 9500000 Gwei
        assert_eq!(stored_sum, sum.0);
    }

    // create mock beacon node instance that implements all defined functions in trait BeaconNode

    struct MockBeaconNode;
    #[async_trait]
    impl BeaconNode for MockBeaconNode {
        async fn get_block_by_block_root(
            &self,
            block_root: &str,
        ) -> Result<Option<BeaconBlock>> {
            Ok(None)
        }

        async fn get_block_by_slot(
            &self,
            slot: Slot,
        ) -> Result<Option<BeaconBlock>> {
            Ok(None)
        }

        async fn get_header(
            &self,
            block_id: &BlockId,
        ) -> Result<Option<BeaconHeaderSignedEnvelope>> {
            Ok(None)
        }

        async fn get_header_by_block_root(
            &self,
            block_root: &str,
        ) -> Result<Option<BeaconHeaderSignedEnvelope>> {
            Ok(None)
        }

        async fn get_header_by_slot(
            &self,
            slot: Slot,
        ) -> Result<Option<BeaconHeaderSignedEnvelope>> {
            Ok(None)
        }

        async fn get_header_by_state_root(
            &self,
            state_root: &str,
            slot: Slot,
        ) -> Result<Option<BeaconHeaderSignedEnvelope>> {
            Ok(None)
        }

        async fn get_last_block(&self) -> Result<BeaconBlock> {
            Err(anyhow!("Not implemented in the MockBeaconNode"))
        }

        async fn get_last_finality_checkpoint(
            &self,
        ) -> Result<FinalityCheckpoint> {
            Err(anyhow!("Not implemented in the MockBeaconNode"))
        }

        async fn get_last_finalized_block(&self) -> Result<BeaconBlock> {
            Err(anyhow!("Not implemented in the MockBeaconNode"))
        }

        async fn get_last_header(&self) -> Result<BeaconHeaderSignedEnvelope> {
            Err(anyhow!("Not implemented in the MockBeaconNode"))
        }

        async fn get_state_root_by_slot(
            &self,
            slot: Slot,
        ) -> Result<Option<StateRoot>> {
            Ok(None)
        }

        async fn get_validator_balances(
            &self,
            state_root: &str,
        ) -> Result<Option<Vec<ValidatorBalance>>> {
            Ok(None)
        }

        async fn get_validators_by_state(
            &self,
            state_root: &str,
        ) -> Result<Vec<ValidatorEnvelope>> {
            // Create some mock validator data to return
            let mock_validators = vec![
                ValidatorEnvelope {
                    status: "active_ongoing".to_string(),
                    validator: Validator {
                        effective_balance: GweiNewtype(32_000_000_000_000_000),
                    },
                },
                ValidatorEnvelope {
                    status: "active_ongoing".to_string(),
                    validator: Validator {
                        effective_balance: GweiNewtype(32_000_000_000_000_000),
                    },
                },
            ];
            Ok(mock_validators)
        }
    }
}
