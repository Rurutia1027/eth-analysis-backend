///! Functions and Data Structures defined for communicate with a BeanconChain node to get various pieces of data.
///! Currently, many calls taking a state_root as input do not acknowledge that a state_root may disappear at any time.
///! They should be updated to do so.
pub mod test_utils;

use anyhow::{anyhow, Result};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::Utc;
use mockall::automock;
use reqwest::StatusCode;
use serde::Deserialize;
use std::fmt::Display;

use super::{slot_from_string, Slot};
