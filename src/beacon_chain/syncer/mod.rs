mod slot_rollback;
mod slot_stream;
mod slot_sync;
mod sync_tracker;
mod cache_refresh;
mod state_sync;

use crate::beacon_chain::deposits;
use crate::beacon_chain::slots::SlotRange;
use crate::beacon_chain::syncer::slot_rollback::rollback_slots;
use crate::env::ENV_CONFIG;
use crate::{
    beacon_chain::node::{
        BeaconBlock, BeaconHeaderSignedEnvelope, BeaconNode, BeaconNodeHttp,
        StateRoot, ValidatorBalance,
    },
    beacon_chain::{balances, issuance, slot_from_string, withdrawals, Slot},
    beacon_chain::{blocks, states},
    db::db,
    json_codecs::i32_from_string,
    performance::TimedExt,
};
use anyhow::{anyhow, Result};
use chrono::Duration;
use futures::{stream, SinkExt, Stream, StreamExt};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use sqlx::{Acquire, PgConnection, PgExecutor, PgPool};
use std::{cmp::Ordering, collections::VecDeque};
use tracing::{debug, info, warn};

lazy_static! {
    static ref BLOCK_LAG_LIMIT: Duration = Duration::days(10 * 365);
}



// todo: modify this from streaming into queue operation to debug
pub async fn sync_beacon_states() -> Result<()> {
    info!("syncing beacon states");
    let db_pool = db::get_db_pool("sync-beacon-states", 3).await;
    let beacon_node = BeaconNodeHttp::new();

    // slot stream's non-empty state is the outer loop's cycling condition
    let mut slots_stream = slot_stream::stream_slots_from_last(&db_pool).await;

    // this queue's non-empty state is the inner loop's cycling condition
    let mut slots_queues = VecDeque::<Slot>::new();

    // sync operations are divided amd execute as unit of slots cached in slots_queues
    // sync complete recorder to record the complete progress of the complete synchronize progress
    let mut progress =
        sync_tracker::sync_progress_tracker(&db_pool, &beacon_node).await;

    while let Some(slot_from_stream) = slots_stream.next().await {
        // every 100 slots print the sync progress complete message
        if slot_from_stream.0 % 100 == 0 {
            info!("sync in progress, {}", progress.get_progress_string());
        }

        // append current slot item to queue
        slots_queues.push_back(slot_from_stream);

        // inner while loop && get front slot from queue and handling slot's grained sync job
        while let Some(slot) = slots_queues.pop_front() {
            debug!(%slot, "analyzing next slot on the queue");

            // get current slot's on the chain state_root value
            // and expect this response body should always be able to fetch the corresponding on chain state_root value
            // from beacon chain api endpoint, otherwise, give a panic
            let on_chain_state_root = beacon_node
                .get_state_root_by_slot(slot)
                .await?
                .unwrap_or_else(|| {
                    panic!("expect state_root to exist for slot {slot} to sync from queue")
                });

            // get current slot's off chain db stored state_root value
            let current_slot_stored_state_root =
                states::get_state_root_by_slot(&db_pool, slot).await;

            // Check if the previous slot's state_root matches the previous slot's on-chain state_root value.
            // 1. If the current slot is the initial slot(Slot 0), return true as no it has no previous state_root needs to be checked.
            // 2. Otherwise, retrieve the state_root of slot - 1 from the off-chain database.
            // -  If no state_root exists in the database for slot-1, return false (mismatch)
            // - If it exists, compare it with the on-chain state_root for slot-1.
            //       - If slot-1's on-chain and off-chain state-root match, it means that the data for slot-1 is correctly synced to db, no rollback is needed.
            //       - If they don't match, a rollback is required to ensure data consistency.
            // Rollback Process:
            // - Identify the first slot associated with the mismatched state_root (slot-1), one state_root contains multiple slots, we need to find the last slot from them.
            // - Remove all data linked to that stata_root (blocks, issuance, deposits, withdrawals) from the database.
            // - After rollback, reinsert the affected slots into the processing queue for resynchronization.
            let last_matches = if slot.0 == 0 {
                true
            } else {
                let last_stored_state_root =
                    states::get_state_root_by_slot(&db_pool, slot).await;
                match last_stored_state_root {
                    None => false,
                    Some(last_stored_state_root) => {
                        let previous_on_chain_state_root = beacon_node
                            .get_state_root_by_slot(slot - 1)
                            .await?
                            .expect("expect state slot before current head to exist");
                        last_stored_state_root == previous_on_chain_state_root
                    }
                }
            };

            if current_slot_stored_state_root.is_none() && last_matches {
                // current slot is empty and last state_root matches.
                debug!("no state stored for current slot and last slots state_root matches chain");
                // begin sync from current state and current slot
                state_sync::sync_slot_by_state_root(
                    &db_pool,
                    &beacon_node,
                    &on_chain_state_root,
                    slot,
                )
                .timed("sync_slot_by_state_root")
                .await?;
            } else {
                // we need to roll back all records associated with the current state_root because it is sync not correctly
                // and then re-insert the roll-back slots to the queue to re-sync the slot's associated state_root's data(blocks, issuance ...) from beacon chain
                debug!(
                    ?current_slot_stored_state_root,
                    last_matches,
                    "current slot should be empty, last stored slot state_root should match previous on-chain state_root");
                let last_matching_slot = slot_sync::find_last_matching_slot(
                    &db_pool,
                    &beacon_node,
                    slot - 1,
                )
                .await?;
                let first_invalid_slot = last_matching_slot + 1;
                warn!(slot = last_matching_slot.0, "rolling back to slot");
                // all records associated with slot values that locate in the range of [first_invalid_slot, ...) will be removed from db tables
                rollback_slots(
                    &mut *db_pool.acquire().await?,
                    first_invalid_slot,
                )
                .await?;

                // traverse all roll-back slots and re-insert them back to the queue
                // each slot item in the queue will be converted into sync sub-tasks to fetch remote data and store them to  db tables
                for invalid_slot in (first_invalid_slot.0..=slot.0).rev() {
                    slots_queues.push_front(invalid_slot.into());
                }
            }
        }

        progress.inc_work_done();
    } // outer while loop

    Ok(())
}
