pub mod heal;

use super::Slot;
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
