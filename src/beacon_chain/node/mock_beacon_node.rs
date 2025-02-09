use crate::beacon_chain::node::{
    BeaconBlock, BeaconHeader, BeaconHeaderEnvelope,
    BeaconHeaderSignedEnvelope, BeaconNode, BlockId, CheckpointEnvelope,
    FinalityCheckpoint, FinalityCheckpoints, StateRoot, ValidatorBalance,
    ValidatorBalancesEnvelope, ValidatorEnvelope, ValidatorsEnvelope,
};
use crate::beacon_chain::states::BeaconState;
use crate::beacon_chain::Slot;
use anyhow::{Ok, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Deserializer;
use std::fs;
use std::fs::File;
use std::io::BufReader;

pub struct MockBeaconHttpNode {
    pub state_root: StateRoot,
    pub headers: BeaconHeaderSignedEnvelope,
    pub block: BeaconBlock,
    pub validator_balances: ValidatorBalancesEnvelope,
    pub validators: ValidatorsEnvelope,
    pub finalityCheckpoints: FinalityCheckpoints,
}

pub fn load_beacon_header_from_file(
    file_path: &str,
) -> Result<BeaconHeaderSignedEnvelope> {
    let file_content = fs::read_to_string(file_path)?;

    // parse json into BeaconHeaderSignedEnvelope
    let json_data: serde_json::Value = serde_json::from_str(&file_content)?;
    let beacon_header: BeaconHeaderSignedEnvelope =
        serde_json::from_value(json_data["data"].clone())?;

    Ok(beacon_header)
}

pub fn load_beacon_block_details_from_file(
    file_path: &str,
) -> Result<BeaconBlock> {
    let file_content = fs::read_to_string(file_path)?;

    // parse json into BeaconBlock struct
    let json_data: serde_json::Value = serde_json::from_str(&file_content)?;
    let beacon_block: BeaconBlock =
        serde_json::from_value(json_data["data"]["message"].clone())?;
    Ok(beacon_block)
}

#[derive(Serialize, Deserialize)]
struct StateRootValue {
    root: String,
    slot: Option<Slot>,
}

pub fn load_beacon_state_root_from_file(
    file_path: &str,
) -> Result<StateRootValue> {
    let file_content = fs::read_to_string(file_path)?;

    // parse json into the value of state root
    let json_data: serde_json::Value = serde_json::from_str(&file_content)?;
    let state_root: StateRootValue =
        serde_json::from_value(json_data["data"].clone())?;
    Ok(state_root)
}

pub fn load_validator_balances_from_file(
    file_path: &String,
    max: i32,
) -> Result<ValidatorBalancesEnvelope> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    // create json parser
    let stream =
        Deserializer::from_reader(reader).into_iter::<serde_json::Value>();
    let mut validator_balances = Vec::new();

    for value in stream {
        let record = value?;
        if let Some(data_array) =
            record.get("data").and_then(|item| item.as_array())
        {
            for item in data_array.iter().take(max as usize) {
                let balance: ValidatorBalance =
                    serde_json::from_value(item.clone())?;
                validator_balances.push(balance);

                if validator_balances.len() == max as usize {
                    break;
                }
            }
        }

        if validator_balances.len() >= max as usize {
            // only fetch records = max
            break;
        }
    }

    Ok(ValidatorBalancesEnvelope {
        data: validator_balances,
    })
}

pub fn load_validators_from_file(
    file_path: &String,
    limit: i32,
) -> Result<ValidatorsEnvelope> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    let stream =
        Deserializer::from_reader(reader).into_iter::<serde_json::Value>();
    let mut validator_envelopes = Vec::new();

    for value in stream {
        let record = value?;
        if let Some(data_array) =
            record.get("data").and_then(|item| item.as_array())
        {
            for item in data_array.iter().take(limit as usize) {
                let validator: ValidatorEnvelope =
                    serde_json::from_value(item.clone())?;
                validator_envelopes.push(validator);

                if validator_envelopes.len() == limit as usize {
                    break;
                }
            }
        }

        if validator_envelopes.len() >= limit as usize {
            break;
        }
    }

    Ok(ValidatorsEnvelope {
        data: validator_envelopes,
    })
}

pub fn load_finality_checkpoints_from_file(
    file_path: &String,
) -> Result<FinalityCheckpoints> {
    let file_content = fs::read_to_string(file_path)?;

    // parse json into BeaconBlock struct
    let json_data: serde_json::Value = serde_json::from_str(&file_content)?;
    let finality_checkpoint: FinalityCheckpoints =
        serde_json::from_value(json_data["data"].clone())?;
    Ok(finality_checkpoint)
}

impl MockBeaconHttpNode {
    pub fn new() -> MockBeaconHttpNode {
        Self {
            state_root: Self::load_beacon_state_root(),
            headers: Self::load_beacon_headers(),
            validator_balances: Self::load_validator_balances(),
            validators: Self::load_validators(),
            block: Self::load_block(),
            finalityCheckpoints: Self::load_finality_checkpoints(),
        }
    }

    fn load_validators() -> ValidatorsEnvelope {
        let project_root = env!("CARGO_MANIFEST_DIR");
        let beacon_validators_file =
            format!("{project_root}/datasets/beaconchain/validators.json")
                .to_string();
        load_validators_from_file(&beacon_validators_file, 30).unwrap()
    }

    fn load_validator_balances() -> ValidatorBalancesEnvelope {
        let project_root = env!("CARGO_MANIFEST_DIR");
        let beacon_validator_balances_file = format!(
            "{project_root}/datasets/beaconchain/validator_balances.json"
        )
        .to_string();
        load_validator_balances_from_file(&beacon_validator_balances_file, 30)
            .unwrap()
    }

    fn load_beacon_state_root() -> StateRoot {
        let project_root = env!("CARGO_MANIFEST_DIR");
        let beacon_state_root_file =
            format!("{project_root}/datasets/beaconchain/root.json")
                .to_string();
        load_beacon_state_root_from_file(&beacon_state_root_file)
            .unwrap()
            .root as StateRoot
    }

    fn load_beacon_headers() -> BeaconHeaderSignedEnvelope {
        let project_root = env!("CARGO_MANIFEST_DIR");
        let beacon_header_file =
            format!("{project_root}/datasets/beaconchain/block_header.json")
                .to_string();
        load_beacon_header_from_file(&beacon_header_file).unwrap()
    }

    fn load_block() -> BeaconBlock {
        let project_root = env!("CARGO_MANIFEST_DIR");
        let beacon_block_detail_file =
            format!("{project_root}/datasets/beaconchain/block_details.json")
                .to_string();
        load_beacon_block_details_from_file(beacon_block_detail_file.as_str())
            .unwrap()
    }

    fn load_finality_checkpoints() -> FinalityCheckpoints {
        let project_root = env!("CARGO_MANIFEST_DIR");
        let beacon_finality_checkpoint_file = format!(
            "{project_root}/datasets/beaconchain/finality_checkpoints.json"
        )
        .to_string();
        load_finality_checkpoints_from_file(&beacon_finality_checkpoint_file)
            .unwrap()
    }
}
#[async_trait]
impl BeaconNode for MockBeaconHttpNode {
    async fn get_block_by_block_root(
        &self,
        block_root: &str,
    ) -> anyhow::Result<Option<BeaconBlock>> {
        Ok(Some(self.block.clone()))
    }

    async fn get_block_by_slot(
        &self,
        slot: Slot,
    ) -> anyhow::Result<Option<BeaconBlock>> {
        Ok(Some(self.block.clone()))
    }

    async fn get_header(
        &self,
        block_id: &BlockId,
    ) -> anyhow::Result<Option<BeaconHeaderSignedEnvelope>> {
        Ok(Some(self.headers.clone()))
    }

    async fn get_header_by_block_root(
        &self,
        block_root: &str,
    ) -> anyhow::Result<Option<BeaconHeaderSignedEnvelope>> {
        Ok(Some(self.headers.clone()))
    }

    async fn get_header_by_slot(
        &self,
        slot: Slot,
    ) -> anyhow::Result<Option<BeaconHeaderSignedEnvelope>> {
        Ok(Some(self.headers.clone()))
    }

    async fn get_header_by_state_root(
        &self,
        state_root: &str,
        slot: Slot,
    ) -> anyhow::Result<Option<BeaconHeaderSignedEnvelope>> {
        Ok(Some(self.headers.clone()))
    }

    async fn get_last_block(&self) -> anyhow::Result<BeaconBlock> {
        Ok(self.block.clone())
    }

    async fn get_last_finality_checkpoint(
        &self,
    ) -> anyhow::Result<FinalityCheckpoint> {
        Ok(self.finalityCheckpoints.finalized.clone())
    }

    async fn get_last_finalized_block(&self) -> anyhow::Result<BeaconBlock> {
        Ok(self.block.clone())
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
        Ok(Some(self.state_root.clone()))
    }

    async fn get_validator_balances(
        &self,
        state_root: &str,
    ) -> anyhow::Result<Option<Vec<ValidatorBalance>>> {
        Ok(Some(self.validator_balances.data.clone()))
    }

    async fn get_validators_by_state(
        &self,
        state_root: &str,
    ) -> anyhow::Result<Vec<ValidatorEnvelope>> {
        Ok(self.validators.data.clone())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;










    #[tokio::test]
    async fn test_load_beacon_header_from_file() {
        let project_root = env!("CARGO_MANIFEST_DIR");
        let beacon_header_file =
            format!("{project_root}/datasets/beaconchain/block_header.json")
                .to_string();
        let data = load_beacon_header_from_file(&beacon_header_file);
        assert!(data.is_ok());
        assert!(data.unwrap().slot().0 > 0);
    }

    #[tokio::test]
    async fn test_load_block_details_from_file() {
        let project_root = env!("CARGO_MANIFEST_DIR");
        let beacon_block_detail_file =
            format!("{project_root}/datasets/beaconchain/block_details.json")
                .to_string();
        let data =
            load_beacon_block_details_from_file(&beacon_block_detail_file);
        assert!(data.is_ok());
        assert!(data.unwrap().slot.0 > 0);
    }

    // root.json
    #[tokio::test]
    async fn test_load_state_root_from_file() {
        let project_root = env!("CARGO_MANIFEST_DIR");
        let beacon_state_root_file =
            format!("{project_root}/datasets/beaconchain/root.json")
                .to_string();
        let data = load_beacon_state_root_from_file(&beacon_state_root_file);
        assert!(data.is_ok());
        // root value comes from root.json
        assert_eq!(data.unwrap().root, "0x6b580f7cdd251de6e46575fde5ff8ccb1e49d25fa20e0e830f4215258cb2851e")
    }

    #[tokio::test]
    async fn test_load_validator_balances_from_file() {
        // this should support max lines to avoid loading to many records to progress
        let project_root = env!("CARGO_MANIFEST_DIR");
        let beacon_validator_balances_file = format!(
            "{project_root}/datasets/beaconchain/validator_balances.json"
        )
        .to_string();
        let data = load_validator_balances_from_file(
            &beacon_validator_balances_file,
            30,
        );
        assert!(data.is_ok());
        assert!(data.unwrap().data.len() <= 30);
    }

    #[tokio::test]
    async fn test_load_validators_from_file() {
        // this should support loaded max lines to avoid load all records from file
        let project_root = env!("CARGO_MANIFEST_DIR");
        let beacon_validators_file =
            format!("{project_root}/datasets/beaconchain/validators.json")
                .to_string();
        let data = load_validators_from_file(&beacon_validators_file, 30);
        assert!(data.is_ok());
        let validators = data.unwrap().data.clone();
        for validator in validators {
            assert!(validator.effective_balance().0 > 0);
        }
    }

    #[tokio::test]
    async fn test_load_finality_checkpoints_from_file() {
        let project_root = env!("CARGO_MANIFEST_DIR");
        let beacon_finality_checkpoint_file = format!(
            "{project_root}/datasets/beaconchain/finality_checkpoints.json"
        )
        .to_string();
        let data = load_finality_checkpoints_from_file(
            &beacon_finality_checkpoint_file,
        );
        assert!(data.is_ok());
    }
}
