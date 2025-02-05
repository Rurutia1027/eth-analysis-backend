///! handles storage and retrieval of beacon blocks in our DB.
pub mod heal;
use crate::units::GweiNewtype;
use sqlx::{PgExecutor, Row};

use super::{
    node::{BeaconBlock, BeaconHeader, BeaconHeaderSignedEnvelope, BeaconNode},
    Slot,
};

use crate::job::job_progress;
pub use heal::heal_block_hashes;

pub const GENESIS_PARENT_ROOT: &str =
    "0x0000000000000000000000000000000000000000000000000000000000000000";

// query deposit_sum_aggregated field from table beacon_blocks table
// in which block_root:string is the primary key
pub async fn get_deposit_sum_from_block_root(
    executor: impl PgExecutor<'_>,
    block_root: &str,
) -> GweiNewtype {
    sqlx::query!(
        "SELECT deposit_sum_aggregated FROM beacon_blocks WHERE block_root = $1", block_root
    ).fetch_one(executor)
        .await
        .unwrap()
        .deposit_sum_aggregated
        .into()
}

// retrieve withdrawal_sum_aggregated field value from table beacon_blocks
// by providing the primary key value -- block_root
pub async fn get_withdrawal_sum_from_block_root(
    executor: impl PgExecutor<'_>,
    block_root: &str,
) -> GweiNewtype {
    sqlx::query!(
        "
        SELECT
            withdrawal_sum_aggregated
        FROM
            beacon_blocks
        WHERE
            block_root = $1
        ",
        block_root
    )
    .fetch_one(executor)
    .await
    .unwrap()
    .withdrawal_sum_aggregated
    .unwrap_or_default()
    .into()
}

// check from db table beacon_blocks where there is any records with
// the given block_root(block hash in string) value.
pub async fn get_is_hash_known(
    executor: impl PgExecutor<'_>,
    block_root: &str,
) -> bool {
    // if given block hash is genesis the initial block hash value
    // this should always exist return true is ok
    if block_root == GENESIS_PARENT_ROOT {
        return true;
    }

    // otherwise query from the db directly
    sqlx::query(
        "
                SELECT EXISTS(
                    SELECT 1 FROM beacon_blocks
                    WHERE block_root = $1
                )
            ",
    )
    .bind(block_root)
    .fetch_one(executor)
    .await
    .unwrap()
    .get("exists")
}

// insert BeaconBlock into table beacon_block table
pub async fn store_block(
    executor: impl PgExecutor<'_>,
    block: &BeaconBlock,
    deposit_sum: &GweiNewtype,
    deposit_sum_aggregated: &GweiNewtype,
    withdrawal_sum: &GweiNewtype,
    withdrawal_sum_aggregated: &GweiNewtype,
    header: &BeaconHeaderSignedEnvelope,
) {
    sqlx::query!(
        "
        INSERT INTO beacon_blocks (
            block_hash,
            block_root,
            deposit_sum,
            deposit_sum_aggregated,
            withdrawal_sum,
            withdrawal_sum_aggregated,
            parent_root,
            state_root
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8
        )
        ",
        block.block_hash(),
        header.root,
        i64::from(deposit_sum.to_owned()),
        i64::from(deposit_sum_aggregated.to_owned()),
        i64::from(withdrawal_sum.to_owned()),
        i64::from(withdrawal_sum_aggregated.to_owned()),
        header.parent_root(),
        header.state_root(),
    )
    .execute(executor)
    .await
    .unwrap();
}

// delete all records in beacon_blocks with each beacon_blocks#state_root value
// locates in the range of the set that constructed by query results
// from querying from table beacon_states with beacon_state#slot >= given slot value
pub async fn delete_blocks(executor: impl PgExecutor<'_>, greater_than_or_equal: Slot) {
    sqlx::query!(
        "
        DELETE FROM beacon_blocks
        WHERE state_root IN (
            SELECT
                state_root
            FROM
                beacon_states
            WHERE beacon_states.slot >= $1
        )
        ",
        greater_than_or_equal.0
    )
        .execute(executor)
        .await
        .unwrap();
}

// delete single block with state_root locates in the query result
// that it's query result from query table beacon_states value slot value equal to query parameter
pub async fn delete_block(executor: impl PgExecutor<'_>, slot: Slot) {
    sqlx::query(
        "
        DELETE FROM beacon_blocks
        WHERE state_root IN (
            SELECT
                state_root
            FROM
                beacon_states
            WHERE slot = $1
        )
        ",
    )
    .bind(slot.0)
    .execute(executor)
    .await
    .unwrap();
}

#[derive(Debug, PartialEq, Eq)]
pub struct DbBlock {
    block_root: String,
    deposit_sum: GweiNewtype,
    deposit_sum_aggregated: GweiNewtype,
    parent_root: String,
    pub block_hash: Option<String>,
    pub state_root: String,
}

struct BlockDbRow {
    block_root: String,
    deposit_sum: i64,
    deposit_sum_aggregated: i64,
    parent_root: String,
    pub block_hash: Option<String>,
    pub state_root: String,
}

// converted BlockDbRow into DbBlock
impl From<BlockDbRow> for DbBlock {
    fn from(value: BlockDbRow) -> Self {
        Self {
            block_hash: value.block_hash,
            block_root: value.block_root,
            deposit_sum: value.deposit_sum.into(),
            deposit_sum_aggregated: value.deposit_sum_aggregated.into(),
            parent_root: value.parent_root,
            state_root: value.state_root,
        }
    }
}

// get a series of blocks which each slot value <= given query slot value
pub async fn get_block_before_slot(
    executor: impl PgExecutor<'_>,
    less_than: Slot,
) -> DbBlock {
    sqlx::query_as!(
        BlockDbRow,
        "
        SELECT
            block_root,
            beacon_blocks.state_root,
            parent_root,
            deposit_sum,
            deposit_sum_aggregated,
            block_hash
        FROM
            beacon_blocks
        JOIN
            beacon_states ON beacon_blocks.state_root = beacon_states.state_root
        WHERE slot < $1
        ORDER BY slot DESC
        LIMIT 1
        ",
        less_than.0
    )
    .fetch_one(executor)
    .await
    .unwrap()
    .into()
}

// update field of block_hash value in table beacon_blocks
// querying the satisfied records by its block_root value the primary key of table beacon_blocks
pub async fn update_block_hash(
    executor: impl PgExecutor<'_>,
    block_root: &str,
    block_hash: &str,
) {
    sqlx::query!(
        "
        UPDATE beacon_blocks
        SET block_hash = $1
        WHERE block_root = $2
        ",
        block_hash,
        block_root
    )
    .execute(executor)
    .await
    .unwrap();
}

pub async fn get_block_by_slot(
    executor: impl PgExecutor<'_>,
    slot: Slot,
) -> Option<DbBlock> {
    sqlx::query_as!(
        BlockDbRow,
        r#"
        SELECT
            block_root,
            beacon_blocks.state_root,
            parent_root,
            deposit_sum,
            deposit_sum_aggregated,
            block_hash
        FROM
            beacon_blocks
        JOIN beacon_states ON
            beacon_blocks.state_root = beacon_states.state_root
        WHERE
            slot = $1
        "#,
        slot.0
    )
    .fetch_optional(executor)
    .await
    .unwrap()
    .map(|row| row.into())
}

#[cfg(test)]
mod tests {
    use db::db::tests;
    use sqlx::Acquire;

    use super::*;
    use crate::beacon_chain::states::{get_last_state, store_state};
    use crate::{
        beacon_chain::node::{
            BeaconBlockBody, BeaconHeader, BeaconHeaderEnvelope, BeaconNode,
            ExecutionPayload,
        },
        db,
    };
    use crate::beacon_chain::tests::store_test_block;

    pub async fn get_last_block_slot(
        executor: impl PgExecutor<'_>,
    ) -> Option<Slot> {
        sqlx::query!(
            "
            SELECT
                beacon_states.slot
            FROM
                beacon_blocks
            JOIN
                beacon_states
            ON
                beacon_states.state_root = beacon_blocks.state_root
            ORDER BY slot DESC
            LIMIT 1
            ",
        )
        .fetch_optional(executor)
        .await
        .unwrap()
        .map(|row| Slot(row.slot))
    }

    #[tokio::test]
    async fn get_is_genesis_known_test() {
        let mut connection = tests::get_test_db_connection().await;
        let mut transaction = connection.begin().await.unwrap();

        let is_hash_known =
            get_is_hash_known(&mut *transaction, GENESIS_PARENT_ROOT).await;
        assert!(is_hash_known)
    }

    #[tokio::test]
    async fn get_is_hash_known_test() {}

    #[tokio::test]
    async fn get_is_hash_not_known_test() {
        let mut connection = tests::get_test_db_connection().await;
        let mut transaction = connection.begin().await.unwrap();

        let is_hash_known =
            get_is_hash_known(&mut *transaction, "0x-unknown-block-hash").await;
        assert!(!is_hash_known)
    }

    #[tokio::test]
    async fn store_block_test() {
        let mut connection = tests::get_test_db_connection().await;
        let mut transaction = connection.begin().await.unwrap();
        let state_root = "0xblock_test_state_root".to_string();
        let slot = Slot(0);
        store_state(&mut *transaction, &state_root, slot).await;
        store_block(
            &mut *transaction,
            // &BeanBlock
            &BeaconBlock {
                body: BeaconBlockBody {
                    deposits: vec![],
                    execution_payload: None,
                },
                parent_root: GENESIS_PARENT_ROOT.to_string(),
                slot,
                state_root: state_root.clone(),
            },
            // deposit_sum
            &GweiNewtype(0),
            // deposit_sum_aggregated
            &GweiNewtype(0),
            // withdrawal_sum
            &GweiNewtype(0),
            // withdrawal_sum_aggregated
            &GweiNewtype(0),
            // header
            &BeaconHeaderSignedEnvelope {
                root: "0xblock_root".to_string(),
                header: BeaconHeaderEnvelope {
                    message: BeaconHeader {
                        slot,
                        parent_root: GENESIS_PARENT_ROOT.to_string(),
                        state_root: state_root.clone(),
                    },
                },
            },
        )
        .await;

        let is_hash_known =
            get_is_hash_known(&mut *transaction, "0xblock_root").await;
        assert!(is_hash_known);
    }

    #[tokio::test]
    async fn get_last_block_number_none_test() {
        let mut connection = db::db::tests::get_test_db_connection().await;
        let mut transaction = connection.begin().await.unwrap();
        let block_number = get_last_block_slot(&mut *transaction);
        assert!(block_number.await.is_none())
    }

    #[tokio::test]
    async fn get_last_block_number_some_test() {
        assert!(true)
    }

    // this beacon_blocks table record deletion by slot value associates with two table
    // the anchor table: beacon_states stores the state_root and slot value
    // the beacon_blocks table which takes state_root as its primary key
    // deletion is separated two steps:
    // 1. query anchor table -- beacon_states by given slot value, and get its state_root
    // 2. then take the `state_root` value that queried from beacon_states and invoke delete operation in table beacon_blocks

    // so in this test case, we need first insert a record to beacon_states table
    // and take the inserted beacon_states#state_root value to build a beacon_blocks reacord and insert to table beacon_blocks.
    // then during test, it will executed the operations as expected.
    // #[tokio::test]
    #[tokio::test]
    async fn delete_block_test() {
        let mut connection = db::db::tests::get_test_db_connection().await;
        let mut transaction = connection.begin().await.unwrap();

        store_test_block(&mut transaction, "delete_block_test").await;

        let block_slot = get_last_block_slot(&mut *transaction).await;
        assert_eq!(block_slot, Some(Slot(0)));

        delete_blocks(&mut *transaction, Slot(0)).await;

        let block_slot = get_last_block_slot(&mut *transaction).await;
        assert_eq!(block_slot, None);
    }

    #[tokio::test]
    async fn get_block_before_slot_test() {
        let mut connection = tests::get_test_db_connection().await;
        let mut transaction = connection.begin().await.unwrap();
        let test_id_before = "last_block_before_slot_before";
    }

    #[tokio::test]
    async fn get_block_before_missing_slot_test() {
        assert!(true)
    }

    #[tokio::test]
    async fn update_block_hash_test() {
        assert!(true)
    }

    #[tokio::test]
    async fn get_block_by_slot_test() {
        assert!(true)
    }
}
