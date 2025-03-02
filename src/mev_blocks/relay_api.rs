use super::MevBlock;
use crate::units::WeiNewtype;
use async_trait::async_trait;
use mockall::{automock, predicate::*};
use serde::Deserialize;

// Earliest ultra-money relay has data for
pub const EARLIEST_AVAILABLE_SLOT: i32 = 5616303;

#[derive(Deserialize)]
pub struct MaybeMevBlock {
    #[serde(rename = "slotNumber")]
    slot_number: i32,
    #[serde(rename = "blockNumber")]
    block_number: i32,
    #[serde(rename = "blockHash")]
    block_hash: String,
    #[serde(rename = "value")]
    bid: Option<WeiNewtype>,
}

impl TryFrom<MaybeMevBlock> for MevBlock {
    type Error = String;

    fn try_from(value: MaybeMevBlock) -> Result<Self, Self::Error> {
        match value.bid {
            Some(bid) => Ok(MevBlock {
                slot: value.slot_number,
                block_number: value.block_number,
                block_hash: value.block_hash,
                bid,
            }),
            None => Err(format!("No bid for block {}", value.block_number)),
        }
    }
}
