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
pub use node::mock_block::{BeaconBlockBuilder, BeaconHeaderSignedEnvelopeBuilder};

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

#[cfg(test)]
pub mod tests {
    use crate::beacon_chain::blocks::store_block;
    use crate::beacon_chain::node::{BeaconBlock,BeaconHeaderSignedEnvelope, mock_block::{BeaconBlockBuilder, BeaconHeaderSignedEnvelopeBuilder}};
    use crate::beacon_chain::states::store_state;
    use crate::units::GweiNewtype;
    use sqlx::{Acquire, PgConnection};

    pub async fn store_test_block(executor: &mut PgConnection, test_id: &str) {
        let header = BeaconHeaderSignedEnvelopeBuilder::new(test_id).build();
        let block = Into::<BeaconBlockBuilder>::into(&header).build();
        store_custom_test_block(executor, &header, &block).await
    }

    pub async fn store_custom_test_block(
        executor: &mut PgConnection,
        header: &BeaconHeaderSignedEnvelope,
        block: &BeaconBlock,
    ) {
        store_state(
            executor.acquire().await.unwrap(),
            &header.header.message.state_root,
            header.header.message.slot,
        )
            .await;

        store_block(
            executor,
            block,
            &GweiNewtype(0),
            &GweiNewtype(0),
            &GweiNewtype(0),
            &GweiNewtype(0),
            header,
        )
            .await
    }
}
