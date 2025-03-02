#[tokio::main]
pub async  fn main() -> anyhow::Result<()> {
    eth_analysis_backend::check_beacon_state_gaps().await?;
    Ok(())
}