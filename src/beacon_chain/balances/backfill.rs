use crate::beacon_chain::{
    balances, node::BeaconNode, node::BeaconNodeHttp, slots::Slot,
};
use futures::{pin_mut, StreamExt};
use pit_wall::Progress;
use sqlx::PgPool;
use tracing::{debug, info, warn};

const GET_BALANCES_CONCURRENCY_LIMIT: usize = 32;
const SLOTS_PER_EPOCH: i64 = 32;

pub enum Granularity {
    Day,
    Epoch,
    Hour,
    Slot,
}

// this function finds how many records there are in table beacon_validators_balance table with state_root == NULL
// , and also it's associated slot value should be equal to the given slot
// however there is no field in beacon_states that's the reason why we need to use left join
// first query slot from beacon_states table, and get results
// then use the results' state_root join table beacon_validators_balance fields
// to filter the records in beacon_validators_balance that satisfy the query conditon
// then use the COUNT() function to get the slots count value
// finally, converted the slots count into the units by the given Granularity{slots, day, hour or epoch}
// based on the beacon definition: 1 slot = 12 seconds, 32 slots = 1 epoch
async fn estimate_work_todo(
    db_pool: &PgPool,
    granularity: &Granularity,
    from: Slot,
) -> u64 {
    let slots_count = sqlx::query!(
        "
        SELECT
            COUNT(beacon_states.slot) as \"count!\"
        FROM
            beacon_states
        LEFT JOIN beacon_validators_balance ON
            beacon_states.state_root = beacon_validators_balance.state_root
        WHERE
            slot = $1
        AND
            beacon_validators_balance.state_root IS NULL
        ",
        from.0
    )
    .fetch_one(db_pool)
    .await
    .unwrap()
    .count;

    match granularity {
        Granularity::Slot => slots_count,
        // treat an epoch as a window in the stream, each window contains 32 slots
        // and each slot can be treated as a step with 12s in the steam

        //// how many epochs, 32 slots = 1 epoch
        Granularity::Epoch => slots_count * SLOTS_PER_EPOCH,

        // how many hours passed ? 1 slot = 12 second, 1 hour = 3600s / 12s = 300 slots
        Granularity::Hour => slots_count / 300,

        // how many days passed ? 1 slot = 12 seconds, 1 day = 24 * 60 * 3600s / 12s = 7200 slots
        Granularity::Day => slots_count / 7200,
    }
    .try_into()
    .unwrap()
}

// this function is designed and implemented for
// backfill the records in table beacon_validators_balance
// first, we use work_estimate calcualte how many slots that in beacon_validators_balance
// with its state_root field empty, this means that this record's value is missing
// and need to be backfilled by the values queried from beacon api endpoint

// in this function each state_root value missing record will be converted into a query task
// that the task will synchronize value of the state_root from remote beacon endpoint
// and store the value back to the db table of beacon_validators_balance
pub async fn backfill_balances(
    db_pool: &PgPool,
    granularity: &Granularity,
    from: Slot,
) {
    // create beacon endpint request client side
    // and configure with correct beacon url request parameters and address suffixes
    let beacon_node = BeaconNodeHttp::new();

    // invoke estimate_work_todo to get the exactly number of the slots by providing
    // the unit of the garnularity{day, hour, slot, or epoch} and start slot value
    let work_todo = estimate_work_todo(db_pool, granularity, from).await;

    // setup a progress instance and assign the specific progress name to it
    let mut progress = Progress::new("backfill-beacon-balances", work_todo);
    let rows = sqlx::query!(
        "
        SELECT
            beacon_states.state_root,
            beacon_states.slot
        FROM
            beacon_states
        LEFT JOIN beacon_validators_balance ON
            beacon_states.state_root = beacon_validators_balance.state_root
        WHERE
            slot >= $1
        AND
            beacon_validators_balance.state_root IS NULL
        ORDER BY slot DESC
        ",
        from.0,
    )
    .fetch(db_pool);

    // there should be multiple duplicated records selected from the table
    // , and we only keep the first one by the given query granilarity unit
    let rows_filtered = rows.filter_map(|row| async move {
        if let Ok(row) = row {
            match granularity {
                Granularity::Slot => Some(row),
                Granularity::Epoch => {
                    if Slot(row.slot).is_first_of_epoch() {
                        Some(row)
                    } else {
                        None
                    }
                }
                Granularity::Hour => {
                    if Slot(row.slot).is_first_of_hour() {
                        Some(row)
                    } else {
                        None
                    }
                }
                Granularity::Day => {
                    if Slot(row.slot).is_first_of_day() {
                        Some(row)
                    } else {
                        None
                    }
                }
            }
        } else {
            None
        }
    });

    // here we traver each item in the queried filter map
    // and establish data fetching task (beacon balance backfill) one by one
    // since queried records are sorted in DESC order
    let tasks = rows_filtered.map(|row| {
        // each task owns it own beacon query client
        let beacon_node_clone = beacon_node.clone();
        async move {
            let validator_balances = beacon_node_clone
                .get_validator_balances(&row.state_root)
                .await
                .unwrap();
            (row.state_root, row.slot, validator_balances)
        }
    });

    let buffered_tasks = tasks.buffered(GET_BALANCES_CONCURRENCY_LIMIT);
    pin_mut!(buffered_tasks);

    // here we traverse the query results that organized as buffer iterator
    // iterate each result and validate whether they are valid value ,
    // valid value will be remained to balances as Vector of ValidatorBalance : Vec<validatorBalance>
    while let Some((state_root, slot, balances_result)) =
        buffered_tasks.next().await
    {
        let validator_balances = {
            match balances_result {
                Some(validator_balances) => validator_balances,
                None => {
                    // progress has it own work estimate counter calculated by estimate_work_todo at the beginning
                    // here we use progress#inc_work_done to let it acc by 1
                    // once the counter match the estimate_work_todo value, this progress will be regared as finished
                    progress.inc_work_done();
                    continue;
                }
            }
        };

        // accumulate each item's valance value together and finally got the balance_sum value as the final result
        let balance_sum = balances::sum_validator_balances(&validator_balances);

        // here we 'backfill' the final result back to the database table
        // this balances_sum is store in the table of beacon_validators_balance
        balances::store_validators_balance(
            db_pool,
            &state_root,
            slot.into(),
            &balance_sum,
        )
        .await;

        // do not forget inc the finish percentage of the progress
        progress.inc_work_done();

        // print the progress of the given block state_root, and slot's balance aggregated value is finished
        info!("{}", progress.get_progress_string());
    }
}
