pub mod heal;

use super::slots::Slot;
pub use heal::heal_beacon_states;
use sqlx::PgExecutor;

#[derive(PartialEq, Debug)]
pub struct BeaconState {
    pub slot: Slot,
    pub state_root: String,
}

// get the freshest(latest) record from table beacon_states
pub async fn get_last_state(
    executor: impl PgExecutor<'_>,
) -> Option<BeaconState> {
    sqlx::query_as!(
        BeaconState,
        r#"
        SELECT
            beacon_states.state_root,
            beacon_states.slot AS "slot: Slot"
        FROM beacon_states
        ORDER BY slot DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(executor)
    .await
    .unwrap()
}

// save beacon state record to table beacon_states
pub async fn store_state(
    executor: impl PgExecutor<'_>,
    state_root: &str,
    slot: Slot,
) {
    sqlx::query!(
        "
        INSERT INTO
            beacon_states
            (state_root, slot)
        VALUES
            ($1, $2)
        ",
        state_root,
        slot.0,
    )
    .execute(executor)
    .await
    .unwrap();
}

pub async fn get_state_root_by_slot(
    executor: impl PgExecutor<'_>,
    slot: Slot,
) -> Option<String> {
    sqlx::query!(
        "
        SELECT
            state_root
        FROM
            beacon_states
        WHERE
            slot = $1
        ",
        slot.0
    )
    .fetch_optional(executor)
    .await
    .unwrap()
    .map(|row| row.state_root)
}

pub async fn delete_states(
    executor: impl PgExecutor<'_>,
    greater_than_or_equal: Slot,
) {
    sqlx::query!(
        "
        DELETE FROM beacon_states
        WHERE beacon_states.slot >= $1
        ",
        greater_than_or_equal.0
    )
    .execute(executor)
    .await
    .unwrap();
}

pub async fn delete_state(executor: impl PgExecutor<'_>, slot: Slot) {
    sqlx::query!(
        "
        DELETE FROM beacon_states
        WHERE slot = $1
        ",
        slot.0
    )
    .execute(executor)
    .await
    .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::db;
    use sqlx::Connection;

    #[tokio::test]
    async fn store_state_test() {
        let mut connection = db::tests::get_test_db_connection().await;
        let mut transaction = connection.begin().await.unwrap();
        store_state(&mut *transaction, "0xstate_root_value", Slot(5550)).await;
        let state = get_last_state(&mut *transaction).await.unwrap();

        assert_eq!(
            BeaconState {
                slot: Slot(5550),
                state_root: "0xstate_root_value".into()
            },
            state
        );
    }

    #[tokio::test]
    async fn get_last_state_test() {
        let mut connection = db::tests::get_test_db_connection().await;
        let mut transaction = connection.begin().await.unwrap();

        store_state(&mut *transaction, "0xstate_root_1", Slot(772)).await;
        store_state(&mut *transaction, "0xstate_root_2", Slot(881)).await;

        let state = get_last_state(&mut *transaction).await.unwrap();

        assert_eq!(
            state,
            BeaconState {
                slot: Slot(881),
                state_root: "0xstate_root_2".to_string()
            }
        )
    }

    #[tokio::test]
    async fn delete_states_test() {
        let mut connection = db::tests::get_test_db_connection().await;
        let mut transaction = connection.begin().await.unwrap();
        store_state(&mut *transaction, "0xstate_root", Slot(6666666)).await;
        let state = get_last_state(&mut *transaction).await;
        assert!(state.is_some());
        delete_state(&mut *transaction, Slot(6666666)).await;
        let state_query_after = get_last_state(&mut *transaction).await;

        // should be none, cause delete should be work ok
        if state_query_after.is_some() {
            assert!(state_query_after.unwrap().slot.0 != 6666666 as i32)
        }
    }

    #[tokio::test]
    async fn get_state_root_by_slot_test() {
        let mut connection = db::tests::get_test_db_connection().await;
        let mut transaction = connection.begin().await.unwrap();
        store_state(&mut *transaction, "0xtest", Slot(999999)).await;
        let state_root =
            get_state_root_by_slot(&mut *transaction, Slot(999999))
                .await
                .unwrap();
        assert_eq!(state_root, "0xtest");
    }
}
