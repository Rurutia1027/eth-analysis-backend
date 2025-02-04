use crate::{beacon_chain::node::BeaconNode, db, kv_store};
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

pub async fn heal_beacon_states() {
    info!("healing reorged states");

    let db_pool = db::get_db_pool("heal-beacon-states", 1).await;
    let kv_store = kv_store::KVStorePostgres::new(db_pool.clone());
    let job_tracer: JobProgress<'_, Slot> =
        JobProgress::new(HEAL_BEACON_STATES_KEY, &kv_store);
    let beacon_node = BeaconNodeHttp::new();
    let last_slot = beacon_chain::states::get_last_state(&db_pool)
        .await
        .expect("a beacon state should be stored before trying to heal any")
        .slot
        .0;
}
