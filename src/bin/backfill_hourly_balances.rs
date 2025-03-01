use tracing::info;

use eth_analysis_backend::{beacon_chain::backfill::backfill_balances, db};
use eth_analysis_backend::beacon_chain::backfill::Granularity;
use eth_analysis_backend::beacon_chain::Slot;

#[tokio::main]
pub async fn main() {
    info!("back filling hourly beacon balances from 1 hour");
    let db_pool = db::get_db_pool("backfill_hourly_balances", 3).await;
    backfill_balances(&db_pool, &Granularity::Hour, Slot(0)).await;
    info!("don back filling hourly beacon balances");
}