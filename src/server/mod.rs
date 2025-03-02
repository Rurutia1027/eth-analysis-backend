use crate::db::db;
use crate::env;
use crate::health::HealthCheckable;
use crate::kv_store::KVStorePostgres;
use crate::server::caching::Cache;
use crate::server::etag_middleware::middleware_fn;
use crate::server::health::ServerHealth;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{middleware, Extension, Router};
use chrono::{DateTime, Duration, Utc};
use lazy_static::lazy_static;
use log::{error, info};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;
use futures::{try_join, TryFutureExt};
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;

mod caching;
mod etag_middleware;
mod health;

lazy_static! {
    static ref FOUR_SECONDS: Duration = Duration::seconds(4);
    static ref ONE_DAY: Duration = Duration::days(1);
    static ref ONE_MINUTE: Duration = Duration::minutes(1);
}

pub struct State {
    pub cache: Cache,
    pub db_pool: PgPool,
    pub health: ServerHealth,
}

pub type StateExtension = Extension<Arc<State>>;

pub async fn start_server() {
    info!("starting serve fees");
    let started_on: DateTime<Utc> = chrono::Utc::now();
    let db_pool = db::get_db_pool("eth-analysis-server", 3).await;
    let kv_store: KVStorePostgres = KVStorePostgres::new(db_pool.clone());

    let cache = Cache::new_with_data(&kv_store).await;
    info!("cache ready");

    let health = ServerHealth::new(started_on);
    let shared_state = Arc::new(State {
        cache,
        db_pool,
        health,
    });

    info!("health ready");

    let update_cache_thread = caching::update_cache_from_notifications(
        shared_state.clone(),
        &shared_state.db_pool,
    )
    .await;

    let app = Router::new()
        .route(
            "/api/v2/fees/healthz",
            get(|state: StateExtension| async move {
                state.health.health_status().into_response()
            }),
        )
        .layer(
            ServiceBuilder::new()
                .layer(middleware::from_fn(etag_middleware::middleware_fn))
                .layer(CompressionLayer::new())
                .layer(Extension(shared_state)),
        );
    let port = "3002";
    let socket_addr = format!("0.0.0.0:{}", port).parse().unwrap();
    let server_thread =
        axum::Server::bind(&socket_addr).serve(app.into_make_service());

    try_join!(
        update_cache_thread.map_err(|err| error!("{}", err)),
        server_thread.map_err(|err| error!("{}", err))
    )
        .unwrap();
}
