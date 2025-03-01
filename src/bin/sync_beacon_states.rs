use anyhow::Result;
use eth_analysis_backend::{beacon_chain::sync_beacon_states_to_local};

#[tokio::main]
pub async fn main() -> Result<()> {
    sync_beacon_states_to_local().await
}