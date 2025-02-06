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

// sync_lag: calculated from the slot,
// local latest state_root slot as the start_slot
// fetch from beacon api endpoint via the same state_root as the end_slot
// cause slot is approximate 12 s
// based on the start_slot and end_slot value under the same state_root value
// we can calculate the `lag` between local and remote beacon chain
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
    debug!(%sync_lag, "beacon sync lag ");

    let SyncData {
        header_block_tuple,
        validator_balances,
    } = gather_sync_data(beacon_node, state_root, slot, &sync_lag).await?;

    // all data has been fetch and cached in the object of SyncData this object
    // now we begin the transaction, and break down & extract different parts from SyncData fields
    // and store the data to beacon associated tables: beacon_states, beacon_blocks, beacon_issuance and beacon_validators_balance
    // --- begin transaction ---
    let mut transaction = db_pool.begin().await?;

    // match header_block_tuple {}

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
    // refresh update cache, but now we haven't implement this yet
    Ok(())
}
