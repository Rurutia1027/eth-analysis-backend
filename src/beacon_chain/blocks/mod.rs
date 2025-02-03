///! handles storage and retrieval of beacon blocks in our DB.
pub mod heal;
use crate::units::GweiNewtype;
use sqlx::{PgExecutor, Row};

use super::{
    node::{BeaconHeaderSignedEnvelope, BeaconNode},
    Slot,
};

use crate::beacon_chain::node::BeaconBlock;
use crate::job::job_progress;
pub use heal::heal_block_hashes;

pub const GENESIS_PARENT_ROOT: &str =
    "0x0000000000000000000000000000000000000000000000000000000000000000";

pub async fn get_deposit_sum_from_block_root(
    executor: impl PgExecutor<'_>,
    block_root: &str,
) -> GweiNewtype {
    sqlx::query!(
        "SELECT deposit_sum_aggregated FROM beacon_blocks WHERE block_root = $1", block_root
    ).fetch_one(executor)
        .await
        .unwrap()
        .deposit_sum_aggregated
        .into()
}

pub async fn update_block_hash(
    executor: impl PgExecutor<'_>,
    block_root: &str,
    block_hash: &str,
) {
    sqlx::query!(
        "
        UPDATE beacon_blocks
        SET block_hash = $1
        WHERE block_root = $2
        ",
        block_hash,
        block_root
    )
    .execute(executor)
    .await
    .unwrap();
}
