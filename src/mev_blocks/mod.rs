use crate::units::WeiNewtype;
use serde::Deserialize;

mod relay_api;

#[derive(Clone, Deserialize, PartialEq, Debug)]
pub struct MevBlock {
    pub slot: i32,
    pub block_number: i32,
    pub block_hash: String,
    pub bid: WeiNewtype,
}
