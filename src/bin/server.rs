#[tokio::main]
pub async fn main() {
    eth_analysis_backend::server::start_server().await;
}
