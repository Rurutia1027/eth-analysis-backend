use crate::beacon_chain::node::{BeaconBlock, BeaconHeader, BeaconHeaderEnvelope, BeaconHeaderSignedEnvelope, BeaconNode, BlockId, FinalityCheckpoint, StateRoot, ValidatorBalance, ValidatorEnvelope};
use crate::beacon_chain::Slot;
use async_trait::async_trait;

pub struct MockBeaconHttpNode;

impl MockBeaconHttpNode {
    pub fn new() -> MockBeaconHttpNode {
        Self {}
    }
}
#[async_trait]
impl BeaconNode for MockBeaconHttpNode {
    async fn get_block_by_block_root(
        &self,
        block_root: &str,
    ) -> anyhow::Result<Option<BeaconBlock>> {
        todo!()
    }

    async fn get_block_by_slot(
        &self,
        slot: Slot,
    ) -> anyhow::Result<Option<BeaconBlock>> {
        todo!()
    }

    async fn get_header(
        &self,
        block_id: &BlockId,
    ) -> anyhow::Result<Option<BeaconHeaderSignedEnvelope>> {
        todo!()
    }

    async fn get_header_by_block_root(
        &self,
        block_root: &str,
    ) -> anyhow::Result<Option<BeaconHeaderSignedEnvelope>> {
        todo!()
    }

    async fn get_header_by_slot(
        &self,
        slot: Slot,
    ) -> anyhow::Result<Option<BeaconHeaderSignedEnvelope>> {
        todo!()
    }

    async fn get_header_by_state_root(
        &self,
        state_root: &str,
        slot: Slot,
    ) -> anyhow::Result<Option<BeaconHeaderSignedEnvelope>> {
        todo!()
    }

    async fn get_last_block(&self) -> anyhow::Result<BeaconBlock> {
        todo!()
    }

    async fn get_last_finality_checkpoint(
        &self,
    ) -> anyhow::Result<FinalityCheckpoint> {
        todo!()
    }

    async fn get_last_finalized_block(&self) -> anyhow::Result<BeaconBlock> {
        todo!()
    }

    async fn get_last_header(
        &self,
    ) -> anyhow::Result<BeaconHeaderSignedEnvelope> {
        // Mock data
        let mock_header = BeaconHeaderSignedEnvelope {
            root: "mock_block_root_779000".to_string(),
            header: BeaconHeaderEnvelope {
                message: BeaconHeader {
                    slot: Slot(779000),
                    parent_root: "mock_parent_root_456".to_string(),
                    state_root: "mock_state_root_789".to_string(),
                },
            },
        };

        Ok(mock_header)
    }

    async fn get_state_root_by_slot(
        &self,
        slot: Slot,
    ) -> anyhow::Result<Option<StateRoot>> {
        todo!()
    }

    async fn get_validator_balances(
        &self,
        state_root: &str,
    ) -> anyhow::Result<Option<Vec<ValidatorBalance>>> {
        todo!()
    }

    async fn get_validators_by_state(
        &self,
        state_root: &str,
    ) -> anyhow::Result<Vec<ValidatorEnvelope>> {
        todo!()
    }
}
