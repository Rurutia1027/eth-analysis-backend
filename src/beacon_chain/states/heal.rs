use crate::{beacon_chain::node::BeaconNode, db};
use crate::{
    beacon_chain::{self, node::BeaconNodeHttp, sync, Slot},
    job::job_progress::JobProgress,
    kv_store::KVStorePostgres,
};
use pit_wall::Progress;
use std::collections::HashMap;
use tracing::{debug, info, warn};

// The first slot we have stored
const FIRST_SHARED_ETH_SUPPLY_SLOT: Slot = Slot(0);

const HEAL_BEACON_STATES_KEY: &str = "heal-beacon-states";

pub async fn heal_beacon_states() {}
