mod node;

use chrono::{DateTime, Utc};
use lazy_static::lazy_static;

pub type BlockNumber = i32;

pub const LONDON_HARD_FORK_BLOCK_HASH: &str =
    "0x9b83c12c69edb74f6c8dd5d052765c1adf940e320bd1291696e6fa07829eee71";
pub const LONDON_HARD_FORK_BLOCK_NUMBER: BlockNumber = 12965000;
pub const MERGE_BLOCK_NUMBER: i32 = 15_537_394;
#[allow(dead_code)]
pub const TOTAL_TERMINAL_DIFFICULTY: u128 = 58750000000000000000000;

// This number was recorded before we has a rigorous definition of how to combine the execution and
// beacon chains to come up with a precise supply. After a rigorous supply is established for every
// block and slot it would be good to update this number.
#[allow(dead_code)]
const MERGE_SLOT_SUPPLY: WeiNewtype = WeiNewtype(120_521_140_924_621_298_474_538_089);

// Until we have an eth supply calculated by adding together per-block supply deltas, we're using
// an estimate based on glassnode data.
#[allow(dead_code)]
const LONDON_SLOT_SUPPLY_ESTIMATE: WeiNewtype = WeiNewtype(117_397_725_113_869_100_000_000_000);

pub const GENESIS_SUPPLY: WeiNewtype = WeiNewtype(72_009_990_499_480_000_000_000_000);


lazy_static! {
    pub static ref LONDON_HARD_FORK_TIMESTAMP: DateTime<Utc> =
        "2021-08-05T12:33:42Z".parse::<DateTime<Utc>>().unwrap();
    pub static ref PARIS_HARD_FORK_TIMESTAMP: DateTime<Utc> =
        "2022-09-15T06:42:59Z".parse::<DateTime<Utc>>().unwrap();
}

pub use node::BlockHash;
use crate::units::WeiNewtype;
