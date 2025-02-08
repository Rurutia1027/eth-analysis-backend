use crate::beacon_chain::node::{BeaconNode, BeaconNodeHttp};
use crate::beacon_chain::{states, Slot};
use anyhow::{anyhow, Result};
use chrono::Duration;
use sqlx::PgPool;
use tracing::debug;

// calculate two slots (on chain and off chain)'s timestamp lag value
// attention: before can invoke this function, we need to ensure that two slots are belong to the same state_root value
pub async fn get_sync_slot_lag(
    beacon_node: &BeaconNodeHttp,
    syncing_slot: Slot,
) -> Result<Duration> {
    let last_header = beacon_node.get_last_header().await?;
    let last_on_chain_slot = last_header.header.message.slot;
    let last_on_chain_slot_date_time = last_on_chain_slot.date_time();
    let slot_date_time = syncing_slot.date_time();
    Ok(last_on_chain_slot_date_time - slot_date_time)
}

// search db's beacon_states table
// first query state_root value from beacon_states via given starting_candidate value
// second query beacon endpoint to fetch the given starting_candidate's state_root value
// if beacon on chain state value match with the local given slot's state_root value , then the given slot value is the `last_matching_slot` value return
// otherwise, decrease the value of the given slot(starting_candidate) as candidate_slot value and take this `candidate_slot` value
// query -> from local db's beacon-states table's state_root value off-chain
// query -> from remote beacon url endpoint's state_root value  on-chain
// continue compare
pub async fn find_last_matching_slot(
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
