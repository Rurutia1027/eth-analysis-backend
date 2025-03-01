use std::sync::Arc;
use axum::Extension;
use chrono::{DateTime, Duration, Utc};
use lazy_static::lazy_static;
use log::info;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use crate::db::db;
use crate::kv_store::KVStorePostgres;
use crate::server::caching::Cache;

mod caching;


lazy_static!{
    static ref FOUR_SECONDS: Duration = Duration::seconds(4);
    static ref ONE_DAY: Duration = Duration::days(1);
    static ref ONE_MINUTE: Duration = Duration::minutes(1);
}

pub struct State {
    pub cache: Cache,
    pub db_pool: PgPool,

}

pub type StateExtension = Extension<Arc<State>>;


pub async fn start_server() {
    info!("starting serve fees");
    let started_on : DateTime<Utc> = chrono::Utc::now();
    let db_pool =db::get_db_pool("eth-analysis-server", 3).await;
    let kv_store: KVStorePostgres = KVStorePostgres::new(db_pool.clone());

    // todo: init Cache
    info!("cache ready");

    // todo: init health
    info!("health ready");


}