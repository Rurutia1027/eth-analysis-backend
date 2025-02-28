use tracing::info;
use eth_analysis_backend::{db::db, beacon_chain::backfill::backfill_balances};
use eth_analysis_backend::beacon_chain::backfill::Granularity;
use eth_analysis_backend::beacon_chain::FIRST_POST_MERGE_SLOT;

#[tokio::main]
pub async fn main() {
    info!("backfilling beacon balances to london");
    let db_pool = db::get_db_pool("backfill_balances_to_london", 3).await;
    backfill_balances(&db_pool, &Granularity::Slot, FIRST_POST_MERGE_SLOT).await;

    info!("done with backfilling beacon balances to london");
}