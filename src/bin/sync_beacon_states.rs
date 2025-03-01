use anyhow::Result;
use eth_analysis_backend::{beacon_chain::sync_beacon_states};

#[tokio::main]
pub async fn main() -> Result<()> {
    sync_beacon_states().await
}