mod balances;
mod blocks;
mod node;
mod states;
mod sync;
mod units;

use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use serde::Serialize;
pub use units::slot_from_string;
pub use units::Slot;

lazy_static! {
    pub static ref GENESIS_TIMESTAMP: DateTime<Utc> =
        "2020-12-01T12:00:23Z".parse().unwrap();
    pub static ref SHAPELLA_SLOT: Slot = Slot(6209536);
}

pub const FIRST_POST_MERGE_SLOT: Slot = Slot(4700013);
pub const FIRST_POST_LONDON_SLOT: Slot = Slot(1778566);

#[derive(Serialize)]
pub struct GweiInTime {
    pub t: u64,
    pub v: i64,
}

impl From<(DateTime<Utc>, i64)> for GweiInTime {
    fn from((dt, gwei): (DateTime<Utc>, i64)) -> Self {
        GweiInTime {
            t: dt.timestamp().try_into().unwrap(),
            v: gwei,
        }
    }
}
