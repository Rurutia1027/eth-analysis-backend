use crate::beacon_chain::node::{
    BeaconBlock, BeaconHeaderSignedEnvelope, BeaconNode, BeaconNodeHttp,
    StateRoot, ValidatorBalance,
};
use crate::beacon_chain::syncer::{cache_refresh, slot_sync, BLOCK_LAG_LIMIT};
use crate::beacon_chain::{
    balances, blocks, deposits, issuance, states, withdrawals, Slot,
};
use crate::performance::TimedExt;
use anyhow::anyhow;
use chrono::Duration;
use sqlx::PgPool;
use tracing::{debug, warn};

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
) -> anyhow::Result<SyncData> {
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

// this function is also the main entry point of start sync dataset from beacon chain to local
// todo: this function looks so complicated maybe we can deposit it to make it a little easier to test and extend
pub async fn sync_slot_by_state_root(
    db_pool: &PgPool,             // db connection pool
    beacon_node: &BeaconNodeHttp, // beacon chain htp request handler
    state_root: &StateRoot,       // local latest state_root value
    slot: Slot,                   // off chain slot value
) -> anyhow::Result<()> {
    // first we take the off chain slot value send request to beacon chain endpoint
    // to fetch the lag value between local off chain slot and on chain latest slot value
    let sync_lag = slot_sync::get_sync_slot_lag(beacon_node, slot).await?;

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
        cache_refresh::update_deferrable_analysis(db_pool).await?
    } else {
        debug!("sync not yet caught up with head of chain, skipping deferrable analysis")
    }

    Ok(())
}
