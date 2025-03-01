use tracing::info;
use eth_analysis_backend::{beacon_chain::backfill, db};
use eth_analysis_backend::beacon_chain::backfill::{backfill_balances, Granularity};
use eth_analysis_backend::beacon_chain::FIRST_POST_LONDON_SLOT;

#[tokio::main]
pub async fn main() {
    info!("back filling hourly beacon balances");
    let db_pool = db::get_db_pool("backfill_hourly_balances_to_london", 3).await;
    backfill_balances(&db_pool, &Granularity::Hour, FIRST_POST_LONDON_SLOT).await;
    info!("done back filling hourly beacon balances to london");
}