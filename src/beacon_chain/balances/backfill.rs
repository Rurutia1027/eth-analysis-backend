use crate::beacon_chain::{
    balances, node::BeaconNode, node::BeaconNodeHttp, Slot,
};
use futures::{pin_mut, StreamExt};
use pit_wall::Progress;
use sqlx::PgPool;
use tracing::{debug, info, warn};

const GET_BALANCES_CONCURRENCY_LIMIT: usize = 32;
const SLOTS_PER_EPOCH: i64 = 32;

pub enum Granularity {
    Day,
    Epoch,
    Hour,
    Slot,
}