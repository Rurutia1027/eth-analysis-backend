use pit_wall::Progress;
use crate::beacon_chain::node::{BeaconNode, BeaconNodeHttp};
use crate::beacon_chain::{states, Slot};
use sqlx::{PgExecutor, PgPool};
use tracing::debug;

// calculate the slot lag between on chain slot and local(off chain) slot value
async fn estimate_slots_remaining(
    executor: impl PgExecutor<'_>,
    beacon_node: &BeaconNodeHttp,
) -> i32 {
    // on beacon chain latest slot value (slot value is increase and beacon chain global unique value)
    let last_slot_on_chain = beacon_node.get_last_header().await.unwrap();

    // off chain local recorded latest slot value
    let last_slot_off_chain = states::get_last_state(executor)
        .await
        .map_or(Slot(0), |state| state.slot);

    // calculate how many slots remain to be sync from remote to local
    let lag = last_slot_on_chain.slot().0 - last_slot_off_chain.0;
    debug!("#estimate_slots_remaining {}", lag);
    return lag;
}

pub async fn sync_progress_tracker(
    db_pool: &PgPool,
    beacon_node: &BeaconNodeHttp,
) -> Progress {
    pit_wall::Progress::new(
        "sync beacon states",
        // we use estimate_slots_remaining this function to estimate the lag value between [off-chain-latest-slot, on-chain-latest-slot]
        estimate_slots_remaining(db_pool, beacon_node)
            .await
            .try_into()
            .unwrap(),
    )
}
