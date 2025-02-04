mod sync;
mod units;
mod blocks;
mod node;

use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
pub use units::slot_from_string;
pub use units::Slot;

lazy_static! {
    pub static ref GENESIS_TIMESTAMP: DateTime<Utc> =
        "2020-12-01T12:00:23Z".parse().unwrap();
    pub static ref SHAPELLA_SLOT: Slot = Slot(6209536);
}


pub const FIRST_POST_MERGE_SLOT: Slot = Slot(4700013);
pub const FIRST_POST_LONDON_SLOT: Slot = Slot(1778566);