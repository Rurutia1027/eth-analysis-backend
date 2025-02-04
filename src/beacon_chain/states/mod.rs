pub mod heal;

use super::Slot;
pub use heal::heal_beacon_states;
use sqlx::PgExecutor;
