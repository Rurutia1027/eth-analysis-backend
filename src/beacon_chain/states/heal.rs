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
    let last_checked = job_tracer.get().await;
    let starting_slot = last_checked.unwrap_or(FIRST_SHARED_ETH_SUPPLY_SLOT).0;

    let work_todo: u64 = (last_slot - starting_slot) as u64;
    let mut progress = Progress::new("heal-beacon-states", work_todo);
    let slots = (starting_slot..=last_slot).collect::<Vec<i32>>();

    // here we take the first and last range with step length = 1000
    // query the beacon_states table
    // converted the values into the hash map with
    // key = slot value
    // value = state_root  -- beacon block hash value
    for chunk in slots.chunks(10000) {
        let first = chunk.first().unwrap();
        let last = chunk.last().unwrap();
        let stored_states = sqlx::query!(
            "
            SELECT
                slot,
                state_root
            FROM
                beacon_states
            WHERE
                slot >= $1
            AND
                slot <= $2
            ORDER BY
                slot ASC
            ",
            *first,
            *last
        )
        .fetch_all(&db_pool)
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.slot, row.state_root))
        .collect::<HashMap<i32, String>>();

        // traverse each slot value in the slot range [first, last]
        for slot in *first..=*last {
            let stored_state_root = stored_states.get(&slot).unwrap();

            // query complete state_root the beacon block hash value by sending request to beacon chain API endpoint
            let state_root = beacon_node
                .get_state_root_by_slot(slot.into())
                .await
                .unwrap()
                .expect("expect state_root to exist for historic slots");

            // what we expect is the queried fresh stata_root value match with the map's value
            // if not match we do two things:
            // --> delete the original blocks from table beacon_states, beacon_issuance, and beacon_validators_balance
            //    all tables' associated data records are deleted in one transaction
            // --> re-synchronized the data from beacon chain side to local database tables beacon_states, beacon_issuance, beacon_validators_balance
            if *stored_state_root != state_root {
                warn!(
                    "state root mismatch, rolling back stored and re-syncing"
                );
                todo!("add sync rollback and sync slot here ");
                info!(%slot, "healed state at slot");
            }

            progress.inc_work_done();
        }

        job_tracer.set(&last.into()).await;
        info!("{}", progress.get_progress_string());
    }

    info!("done healing beacon states")
}
