use crate::{
    beacon_chain::{
        blocks, node::BeaconNode, node::BeaconNodeHttp, FIRST_POST_MERGE_SLOT,
    },
    db,
    job::job_progress::JobProgress,
    kv_store,
};
use futures::{try_join, TryStreamExt};
use pit_wall::Progress;
use tracing::{debug, info};

const HEAL_BLOCK_HASHES_KEY: &str = "heal-block-hashes";

pub async fn heal_block_hashes() {
    info!("healing execution block hashes");
    let db_pool = db::get_db_pool("heal-beacon-states", 1).await;
    let kv_store = kv_store::KVStorePostgres::new(db_pool.clone());
    let job_tracker = JobProgress::new(HEAL_BLOCK_HASHES_KEY, &kv_store);
    let beacon_node = BeaconNodeHttp::new();
    // fetch the first slot value from job tracker, if fetch nothing use FIRST_POST_MERGE_SLOT instead
    let first_slot = job_tracker.get().await.unwrap_or(FIRST_POST_MERGE_SLOT);

    let work_todo = sqlx::query!(
        r#"
            SELECT
                COUNT (*) as "count!"
            FROM
                beacon_blocks
            JOIN beacon_states ON
                beacon_blocks.state_root = beacon_states.state_root
            WHERE
                slot >= $1
            AND
                block_hash IS NULL
        "#,
        first_slot.0
    )
    .fetch_one(&db_pool)
    .await
    .unwrap();

    // create local temp struct to store query data as struct
    struct BlockSlotRow {
        block_root: String,
        slot: i32,
    }

    let mut rows = sqlx::query_as!(
        BlockSlotRow,
        r#"
        SELECT
            block_root,
            slot AS "slot!"
        FROM
            beacon_blocks
        JOIN beacon_states ON
            beacon_blocks.state_root = beacon_states.state_root
        WHERE
            slot >= $1
        "#,
        first_slot.0
    )
    .fetch(&db_pool);

    // `work_todo` is a query that counts how many rows in the `beacon_blocks` table need to be processed.
    // specifically where `block_hash` is NULL and the `slot` is greater than or equal to `first_slot.0`.
    // This count is used to track the total number of blocks that require "healing"(i.e., updating the block hash).
    // We use this count to initialize the progress tracker, ensuring the healing process can report progress as it
    //processes each block.
    let mut progress =
        Progress::new("heal-block-hashes", work_todo.count.try_into().unwrap());

    while let Some(row) = rows.try_next().await.unwrap() {
        let block_root = row.block_root;
        let slot = row.slot;

        let block = beacon_node
            .get_block_by_block_root(&block_root)
            .await
            .unwrap()
            .expect("expect block to exist for historic block_root");

        let block_hash = block
            .body
            .execution_payload
            .expect("expect execution payload to exist for post-merge block")
            .block_hash;

        debug!(block_root, block_hash, "setting block hash");

        blocks::update_block_hash(&db_pool, &block_root, &block_hash).await;

        progress.inc_work_done();

        if slot % 100 == 0 {
            info!("{}", progress.get_progress_string());
            job_tracker.set(&slot.into()).await;
        }
    }

    info!("done healing beacon block hashes")
}
