use crate::beacon_chain::deposits;
use crate::beacon_chain::slots::SlotRange;
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
    static ref BLOCK_LAG_LIMIT: Duration = Duration::minutes(5);
}

struct SyncData {
    header_block_tuple: Option<(BeaconHeaderSignedEnvelope, BeaconBlock)>,
    validator_balances: Option<Vec<ValidatorBalance>>,
}

// Slot in the Ethereum Beacon Chain is a globally unique, monotonically increasing number.
// It does not reset when the state_root changes. Even if the beacon chain state updates,
// the slot count continues to increment without restarting from zero.
// sync_lag: calculated from the slot,
// local latest state_root slot as the start_slot
// fetch from beacon api endpoint via the same state_root as the end_slot
// cause slot is approximate 12 s , we can calculate the `lag` between local and remote beacon chain
// slot is beacon chain global unique increase value, and this value will not be reset when state root modifies
async fn gather_sync_data(
    beacon_node: &BeaconNodeHttp,
    state_root: &StateRoot,
    slot: Slot,
    sync_lag: &Duration,
) -> Result<SyncData> {
    let header = beacon_node.get_header_by_slot(slot).await?;
    let state_root_check = beacon_node
        .get_state_root_by_slot(slot)
        .await?
        .expect("expect state_root to be available for currently syncing slot");

    // state_root: is our db beacon_states' fetched latest/freshest state_root value
    // state_root_check: is beacon api endpoint's fetched latest state_root value
    // if they are not match it means there are some blocks updated and the status of the beacon chain updated
    // it means our local db stored the latest state is not the 'latest'
    // we cannot execute the synchronize option among different state_root values (local != remote)
    // so return with error message, and manually update the remote latest state_root value to db then re-trigger the sync operation
    if *state_root != state_root_check {
        return Err(anyhow!(
            "slot reorged during gather_sync_data phase, can't continue sync of current state_root {}",
            state_root
        ));
    }

    // 1. use match pattern validate received header is a valid BeaconHeaderSignedEnvelope
    // 2. if validated ok, then extract block_root(block hash) value from the header
    // 3. take the block_root value send request to beacon API endpoint to fetch the complete BeaconBlock message
    // 4. encapsulate request header and response data block in tuple item to header_block_tuple:  Option<(BeaconHeaderSignedEnvelope, BeaconBlock)>
    let header_block_tuple = match header {
        None => None,
        Some(header) => {
            let block = beacon_node
                .get_block_by_block_root(&header.root)
                .await?
                .unwrap_or_else(|| {
                    panic!("expect a block to be available for currently syncing block_root {}", header.root)
                });
            Some((header, block))
        }
    };

    // after sync BeaconBlock ok, we continue with the Validator Balances -- this is a vector of ValidatorBalance items
    // anyhow it has lots of records
    let validator_balances = {
        // BLOCK_LAG_LIMIT is the threshold value set in this project
        // it will disable the synchronization once the lag is too long
        // ---> too many records of validator_balances to sync it will consume too many resources and spend too much time
        if sync_lag > &BLOCK_LAG_LIMIT {
            // todo: BLOCK_LAG_LIMIT can be designed via hot loading, so that once the system's resource is limit or response time too long we can modify it
            // todo: or it can be integrated with some auto monitor tool like prometheus some stuff -- that would be interesting !!
            warn!(
                %sync_lag,
                "block lag over limit, skipping get_validator_balances"
            );
            // return None without trigger data sync
            None
        } else {
            // take the state_root -- latest state_root value in beacon_states table
            let validator_balances = beacon_node
                .get_validator_balances(state_root)
                .await?
                .expect("expect validator balances to exist for the given state_root");
            Some(validator_balances)
        }
    };

    // when two fetch beacon api endpoints return ok
    // take the two queried objects create SyncData and then return
    let sync_data = SyncData {
        header_block_tuple,
        validator_balances,
    };

    Ok(sync_data)
}

// calculate two slots (on chain and off chain)'s timestamp lag value
// attention: before can invoke this function, we need to ensure that two slots are belong to the same state_root value
async fn get_sync_lag(
    beacon_node: &BeaconNodeHttp,
    off_chain_slot: Slot,
) -> Result<Duration> {
    let last_on_chain_header = beacon_node.get_last_header().await?;
    let last_on_chain_slot = last_on_chain_header.header.message.slot;
    let last_on_chain_slot_ts = last_on_chain_slot.date_time();
    let off_chain_slot_ts = off_chain_slot.date_time();
    let lag = last_on_chain_slot_ts - off_chain_slot_ts;
    Ok(lag)
}

// this function is also the main entry point of start sync dataset from beacon chain to local
// todo: this function looks so complicated maybe we can deposit it to make it a little easier to test and extend
pub async fn sync_slot_by_state_root(
    db_pool: &PgPool,             // db connection pool
    beacon_node: &BeaconNodeHttp, // beacon chain htp request handler
    state_root: &StateRoot,       // local latest state_root value
    slot: Slot,                   // off chain slot value
) -> Result<()> {
    // first we take the off chain slot value send request to beacon chain endpoint
    // to fetch the lag value between local off chain slot and on chain latest slot value
    let sync_lag = get_sync_lag(beacon_node, slot).await?;

    let SyncData {
        header_block_tuple,
        validator_balances,
    } = gather_sync_data(beacon_node, state_root, slot, &sync_lag).await?;

    // all data has been fetch and cached in the object of SyncData this object
    // now we begin the transaction, and break down & extract different parts from SyncData fields
    // and store the data to beacon associated tables: beacon_states, beacon_blocks, beacon_issuance and beacon_validators_balance
    // --- begin transaction ---
    let mut transaction = db_pool.begin().await?;

    match header_block_tuple {
        None => {
            debug!(
                "storing slot without block, slot: {:?}, state_root: {}",
                slot, state_root
            );
            states::store_state(&mut *transaction, state_root, slot)
                .timed("store state without block")
                .await;
        }
        Some((ref header, ref block)) => {
            // calculate block's total input aggregated value,
            // first fetch block's parent aggregated sum value
            // then traverse each record in current block accumulate each record's value then return
            let deposit_sum_aggregated =
                deposits::get_deposit_sum_aggregated(&mut *transaction, block)
                    .await;

            // calculate block's total output aggregated values,
            // first fetch block's parent aggregated sum value,
            // then traverse each record in current block accumulate each record's value then return
            let withdrawal_sum_aggregated =
                withdrawals::get_withdrawal_sum_aggregated(
                    &mut *transaction,
                    block,
                )
                .await;

            // find current block's parent_root (parent hash value)
            // from table beacon_blocks
            let is_parent_known = blocks::get_is_hash_known(
                &mut *transaction,
                &header.parent_root(),
            )
            .await;

            // if current block's parent block hash not exist in local table
            // throw error message
            if !is_parent_known {
                return Err(anyhow!(
            "trying to insert beacon block with missing parent, block_root: {}, parent_root: {:?}",
            header.root,
            header.header.message.parent_root
        ));
            }

            // save on beacon chain fetched state_root(latest) and slot value to beacon_states table
            states::store_state(
                &mut *transaction,
                &header.state_root(),
                header.slot(),
            )
            .await;

            // after the on chain state_root value this anchor is saved, we continue store on chain fetched beacon block
            blocks::store_block(
                &mut *transaction,
                block,
                // invoke deposits function to calculate each deposit record deposit amount in current block
                &deposits::get_deposit_sum_from_block(block),
                &deposit_sum_aggregated, // current block deposits' amount + block's parent deposit aggregated sum
                // invoke withdrawals inner defined functions to calculate each withdrawal amount in current block
                &withdrawals::get_withdrawal_sum_from_block(block),
                &withdrawal_sum_aggregated, // current block withdrawals' amount + block's parent withdrawals aggregated sum
                header,
            )
            .await;
        }
    }

    // after finish store beacon_states and beacon_blocks
    // we continue with storing validator_balances value to corresponding beacon table
    // only pattern match will store operation allowed
    if let Some(ref validator_balances) = validator_balances {
        debug!("validator balances present");
        let validator_balances_sum =
            balances::sum_validator_balances(validator_balances);
        balances::store_validators_balance(
            &mut *transaction,
            state_root,
            slot,
            &validator_balances_sum,
        )
        .await;

        if let Some((_, block)) = header_block_tuple {
            let deposit_sum_aggregated =
                deposits::get_deposit_sum_aggregated(&mut *transaction, &block)
                    .await;
            let withdrawal_sum_aggregated =
                withdrawals::get_withdrawal_sum_aggregated(
                    &mut *transaction,
                    &block,
                )
                .await;

            issuance::store_issuance(
                &mut *transaction,
                state_root,
                slot,
                &issuance::calc_issuance(
                    &validator_balances_sum,
                    &withdrawal_sum_aggregated,
                    &deposit_sum_aggregated,
                ),
            )
            .await;
        }

        // todo! update the latest slot value to db table , but this haven't finish yet
        // leave a todo here
    }

    // --- end transaction ---
    transaction.commit().await?;

    // here we fetch the beacon chain latest state_root value
    // and compare it with our local state_root value
    let last_on_chain_state_root = beacon_node
        .get_last_header()
        .await?
        .header
        .message
        .state_root;

    if last_on_chain_state_root == *state_root {
        debug!(
            "sync caught up with head of chain, updating deferrable analysis"
        );
        update_deferrable_analysis(db_pool).await?
    } else {
        debug!("sync not yet caught up with head of chain, skipping deferrable analysis")
    }

    Ok(())
}

async fn update_deferrable_analysis(db_pool: &PgPool) -> Result<()> {
    // todo : refresh update cache, but now we haven't implement this yet
    Ok(())
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct HeadEvent {
    #[serde(deserialize_with = "slot_from_string")]
    slot: Slot,
    block: String,
    state: String,
}

// extract required fields from BeaconHeaderSignedEEnvelope
// to initialize instance of HeadEvent
impl From<BeaconHeaderSignedEnvelope> for HeadEvent {
    fn from(envelop: BeaconHeaderSignedEnvelope) -> Self {
        Self {
            state: envelop.header.message.state_root,
            block: envelop.root,
            slot: envelop.header.message.slot,
        }
    }
}

/**
This function takes a starting slot (`start_slot`) as input and streams all slot numbers within the range [start_slot, end_slot],
where the `end_slot` is dynamically determined by the latest `head.slot` received from the Beacon API's event stream.
The function subscribes to the Beacon API's `head` event stream, which provides the latest slot numbers as they are confirmed.

It checks for any gaps between the received slots and fills them in accordingly.

The valid slot numbers are then sent concurrently into a buffer using the `tx` (write) channel, allowing for multiple threads
to perform this operation.

Finally, the `tx` channel is released, and the `rx` (read) channel is returned to the caller.
The caller can then iterate over the buffer via the `rx` handler to access the slot numbers as they are processed.
*/
async fn stream_slots(slot_to_follow: Slot) -> impl Stream<Item = Slot> {
    let beacon_url = ENV_CONFIG
        .beacon_url
        .as_ref()
        .expect("BEACON_URL is required for env to stream beacon updates");
    let url_string = format!("{beacon_url}/eth/v1/events/?topics=head");
    let url = reqwest::Url::parse(&url_string).unwrap();

    // client created for subscribe event stream from beacon API endpoint
    let client = eventsource::reqwest::Client::new(url);

    // create a buffer space with buffer write channel as tx and read channel as rx
    let (mut tx, rx) = futures::channel::mpsc::unbounded();

    tokio::spawn(async move {
        let mut last_slot = slot_to_follow;

        // Events received from the client might not arrive in strict sequential order, and gaps between slot values may occur.
        // To handle this, we detect gaps between the received head.slot and the last known local slot, and fill in the missing slots accordingly.
        for event in client {
            // subscribed event item from remote
            let event = event.unwrap();

            // use pattern match filter event type we care about
            match event.event_type {
                Some(ref event_type) if event_type == "head" => {
                    let head =
                        serde_json::from_str::<HeadEvent>(&event.data).unwrap();

                    // header event's beacon latest slot value -> head.slot
                    // local begin sync slot value -> slot_to_follow = last_slot
                    // take this if expression to check there exists gap between two slots: head.slot and last_slot
                    if head.slot > last_slot && head.slot != last_slot + 1 {
                        for missing_slot in (last_slot + 1).0..head.slot.0 {
                            debug!(
                                missing_slot,
                                "add missing slot to slots stream"
                            );
                            // appending missing slot that located between [last_slot, head.slot] via buffer write channel handler
                            tx.send(Slot(missing_slot)).await.unwrap();
                        }
                    }
                    // update last_slot value, and continue process next event's header slot value
                    last_slot = head.slot;
                    tx.send(head.slot).await.unwrap();
                }

                Some(event) => {
                    warn!(event, "received an event from server that wes not head event, discard it!")
                }

                None => {
                    debug!("received an empty server event, discard it!")
                }
            }
        }
    });
    rx
}

// after we fetch the start slot value from db or init value of Slot(0)
// next we query from the beacon endpoint to extract the remote slot value from the latest header message
// gte_slot --> our local latest slot value, and it is also the start slot
// value we gonna fetch from the remote beacon endpoint [start = gte_slot, end = last_slot_on_start]
async fn stream_slots_from(gte_slot: Slot) -> impl Stream<Item = Slot> {
    debug!("streaming slots from {gte_slot}");

    let beacon_node = BeaconNodeHttp::new();

    // extract the remote slot value from the latest header message as the last_slot_on_start
    let last_slot_on_start = beacon_node
        .get_last_header()
        .await
        .unwrap()
        .header
        .message
        .slot;

    debug!("last slot on chain: {}", &last_slot_on_start);
    let slots_stream = stream_slots(last_slot_on_start).await;

    // slot_range => [start_slot = gte_slot, end_slot = last_slot_on_start]
    let slot_range = SlotRange::new(gte_slot, last_slot_on_start);

    let historic_slots_stream = stream::iter(slot_range);
    historic_slots_stream.chain(slots_stream)
}

async fn stream_slots_from_last(db_pool: &PgPool) -> impl Stream<Item = Slot> {
    // before we start to fetch data from beacon endpoints
    // we first fetch local db table beacon_states to get the latest/freshest record value and extract record's slot value,
    // let's say the LOCAL_LATEST_SLOT_VALUE
    // if no records exists in the db table beacon_states, we take Slot(0) as the slot value
    // let's say the LOCAL_LATEST_SLOT_VALUE
    let last_synced_state = states::get_last_state(db_pool).await;
    let next_slot_to_sync =
        last_synced_state.map_or(Slot(0), |state| state.slot + 1);

    // there we already go the next_slot_to_sync the LOCAL_LATEST_SLOT_VALUE
    // then we got the next slot value to be sync from beacon endpoint is LOCAL_LATEST_SLOT_VALUE + 1
    stream_slots_from(next_slot_to_sync).await
}

// this function will delete multiple records from beacon tables,
// that the records locates by the given slot range [given_slot, ...)
pub async fn rollback_slots(
    executor: &mut PgConnection,
    greater_than_or_equal: Slot,
) -> Result<()> {
    debug!("rolling back data based on slots locates in range of [{greater_than_or_equal}, ...]");
    let mut transaction = executor.begin().await?;
    // todo: update table eth_supply but we haven't implement this table's associated function yet, leave a todo here
    blocks::delete_blocks(&mut *transaction, greater_than_or_equal).await;
    issuance::delete_issuances(&mut *transaction, greater_than_or_equal).await;
    balances::delete_validator_sums(&mut *transaction, greater_than_or_equal)
        .await;
    states::delete_states(&mut *transaction, greater_than_or_equal).await;
    transaction.commit().await?;
    Ok(())
}

// this function will delete records from multiple beacon tables
// that the records in the beacon tables share the same slot value provided by the parameter
pub async fn rollback_slot(
    executor: &mut PgConnection,
    slot: Slot,
) -> Result<()> {
    debug!("rolling back data from db tables based on the given slot {slot}");
    let mut transaction = executor.begin().await?;
    // todo: update table eth_supply but we haven't implement this table's associated function yet, leave a todo here
    // first - delete block record in beacon_blocks table that the block locates in the given slot period(12 s) on beacon chain
    blocks::delete_block(&mut *transaction, slot).await;

    // second - delete issuance records in beacon_issuance table
    issuance::delete_issuance(&mut *transaction, slot).await;

    // third - delete validator sum from beacon_validators_balance tabel
    balances::delete_validator_sum(&mut *transaction, slot).await;

    // last -- delete record from table beacon_states -- this should be the last delete, because the above table deletion all refers to
    // record in beacon_states
    states::delete_state(&mut *transaction, slot).await;
    transaction.commit().await?;
    Ok(())
}

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

// search db's beacon_states table
// first query state_root value from beacon_states via given starting_candidate value
// second query beacon endpoint to fetch the given starting_candidate's state_root value
// if beacon on chain state value match with the local given slot's state_root value , then the given slot value is the `last_matching_slot` value return
// otherwise, decrease the value of the given slot(starting_candidate) as candidate_slot value and take this `candidate_slot` value
// query -> from local db's beacon-states table's state_root value off-chain
// query -> from remote beacon url endpoint's state_root value  on-chain
// continue compare
async fn find_last_matching_slot(
    db_pool: &PgPool,
    beacon_node: &BeaconNodeHttp,
    starting_candidate: Slot,
) -> Result<Slot> {
    let mut candidate_slot = starting_candidate;
    let mut off_chain_state_root =
        states::get_state_root_by_slot(db_pool, candidate_slot).await;

    // take the init slot value query beacon chain to get the given slot's state_root value from beacon chain's response message
    let mut on_chain_state_root = beacon_node
        .get_header_by_slot(candidate_slot)
        .await?
        .map(|envelope| envelope.header.message.state_root);

    loop {
        match (off_chain_state_root, on_chain_state_root) {
            (Some(off_chain_state_root), Some(on_chain_state_root))
                if off_chain_state_root == on_chain_state_root =>
            {
                debug!(off_chain_state_root, on_chain_state_root, "off-chain and on-chain state root value match by given slot: {candidate_slot}");
                break;
            }

            _ => {
                // refresh the candidate_slot minus it by 1
                candidate_slot = candidate_slot - 1;

                // continue query off chain state_root value via the new candidate_slot  --> local db table beacon-_states
                off_chain_state_root =
                    states::get_state_root_by_slot(db_pool, candidate_slot)
                        .await;

                // continue query on chain state_root value via the new candidate_slot --> parse from beacon endpoint response message
                on_chain_state_root = beacon_node
                    .get_header_by_slot(candidate_slot)
                    .await?
                    .map(|msg| msg.header.message.state_root);
            }
        }
    } // loop

    debug!(
        slot = candidate_slot.0,
        "found a state match between stored and on-chain"
    );
    Ok(candidate_slot)
}

pub async fn sync_beacon_states() -> Result<()> {
    info!("syncing beacon states");
    let db_pool = db::get_db_pool("sync-beacon-states", 3).await;
    sqlx::migrate!("../../../").run(&db_pool).await.unwrap();
    let beacon_node = BeaconNodeHttp::new();

    // slot stream's non-empty state is the outer loop's cycling condition
    let mut slots_stream = stream_slots_from_last(&db_pool).await;

    // this queue's non-empty state is the inner loop's cycling condition
    let mut slots_queues = VecDeque::<Slot>::new();

    // sync operations are divided amd execute as unit of slots cached in slots_queues
    // sync complete recorder to record the complete progress of the complete synchronize progress
    let mut progress = pit_wall::Progress::new(
        "sync beacon states",
        // we use estimate_slots_remaining this function to estimate the lag value between [off-chain-latest-slot, on-chain-latest-slot]
        estimate_slots_remaining(&db_pool, &beacon_node)
            .await
            .try_into()
            .unwrap(),
    );

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
                sync_slot_by_state_root(
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
                let last_matching_slot =
                    find_last_matching_slot(&db_pool, &beacon_node, slot - 1)
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
