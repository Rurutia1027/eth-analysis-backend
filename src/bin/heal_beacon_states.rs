#[tokio::main]
pub async fn main() {
    eth_analysis_backend::beacon_chain::heal_beacon_states().await;
}
