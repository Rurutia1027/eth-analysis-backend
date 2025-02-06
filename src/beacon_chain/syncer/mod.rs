use anyhow::{anyhow, Result};
use chrono::Duration;
use futures::{stream, SinkExt, Stream, StreamExt};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use sqlx::{Acquire, PgConnection, PgExecutor, PgPool};
use std::{cmp::Ordering, collections::VecDeque};
use tracing::{debug, info, warn};
use crate::env::ENV_CONFIG;
use crate::{
    beacon_chain::{withdrawals, balances, issuance, slot_from_string, Slot },
    beacon_chain::node::{BeaconBlock, BeaconNode, BeaconNodeHttp, StateRoot, ValidatorBalance, BeaconHeaderSignedEnvelope},
    beacon_chain::{states, blocks},
    db::db,
    json_codecs::i32_from_string,
    performance::TimedExt,
};

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

    // state_root: is our db beacon_state's fetched latest/freshest state_root value
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













