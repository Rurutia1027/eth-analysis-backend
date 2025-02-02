use chrono::{DateTime, Utc};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref LONDON_HARD_FORK_TIMESTAMP: DateTime<Utc> =
        "2021-08-05T12:33:42Z".parse::<DateTime<Utc>>().unwrap();
    pub static ref PARIS_HARD_FORK_TIMESTAMP: DateTime<Utc> =
        "2022-09-15T06:42:59Z".parse::<DateTime<Utc>>().unwrap();
}
