use super::{State, StateExtension};
use crate::caching::{CacheKey, ParseCacheKeyError};
use crate::{
    caching,
    env::ENV_CONFIG,
    kv_store::{KVStorePostgres, KvStore},
};
use axum::{
    http::{HeaderMap, HeaderValue},
    response::IntoResponse,
    Extension, Json,
};
use chrono::Duration;
use enum_iterator::all;
use futures::{Stream, TryStreamExt};
use lazy_static::lazy_static;
use reqwest::{header, StatusCode};
use serde_json::Value;
use sqlx::{postgres::PgNotification, PgPool};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use thiserror::Error;
use tokio::task::JoinHandle;
use tracing::{debug, info, trace, warn};

pub struct Cache(RwLock<HashMap<CacheKey, Value>>);

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

impl Cache {
    pub fn new() -> Self {
        Self(RwLock::new(HashMap::new()))
    }

    async fn load_from_db(&self, kv_store: &impl KvStore) {}

    pub async fn new_with_data(kv_store: &impl KvStore) -> Self {
        let cache: Cache = Self::new();
        cache.load_from_db(kv_store).await;
        cache
    }
}

pub async fn cached_get_with_custom_duration(
    Extension(state): StateExtension,
    analysis_cache_key: &CacheKey,
    max_age: &Duration,
    // s_max_age: Option<u32>,
    stale_while_revalidate: &Duration,
) -> impl IntoResponse {
    let mut headers = HeaderMap::new();

    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_str(&format!(
            "public, max-age={}, stale-while-revalidate={}",
            max_age.num_seconds(),
            stale_while_revalidate.num_seconds()
        ))
        .unwrap(),
    );

    match state.cache.0.read().unwrap().get(analysis_cache_key) {
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
        Some(cached_value) => {
            (headers, Json(cached_value).into_response()).into_response()
        }
    }
}

lazy_static! {
    static ref SIX_SECONDS: Duration = Duration::seconds(6);
    static ref TWO_MINUTES: Duration = Duration::seconds(120);
}

pub async fn cached_get(
    state: StateExtension,
    analysis_cache_key: &CacheKey,
) -> impl IntoResponse {
    cached_get_with_custom_duration(
        state,
        analysis_cache_key,
        &SIX_SECONDS,
        &TWO_MINUTES,
    )
    .await
}

async fn process_notifications(
    mut notification_stream: impl Stream<Item = Result<PgNotification, sqlx::Error>>
        + Unpin,
    state: Arc<State>,
    kv_store: impl KvStore,
) {
    while let Some(notification) = notification_stream.try_next().await.unwrap()
    {
        let payload = notification.payload();

        match payload.parse::<CacheKey>() {
            Err(ParseCacheKeyError::UnknownCacheKey(cache_key)) => {
                trace!(
                    %cache_key,
                    "unspported cache update, skipping"
                );
            }
            Ok(cache_key) => {
                let value = caching::get_serialized_caching_value(
                    &kv_store, &cache_key,
                )
                .await;
                if let Some(value) = value {
                    state.cache.0.write().unwrap().insert(cache_key, value);
                } else {
                    warn!(
                        %cache_key,
                        "got a message to update our served cache, but DB had no value to give"
                    );
                }

                // todo: update state health status
            }
        }
    }
}

pub async fn update_cache_from_notifications(
    state: Arc<State>,
    db_pool: &PgPool,
) -> JoinHandle<()> {
    let db_url = format!(
        "{}?application_name={}",
        ENV_CONFIG.db_url, "serve-rs-cache-update"
    );
    let mut listener =
        sqlx::postgres::PgListener::connect(&db_url).await.unwrap();
    listener.listen("cache-update").await.unwrap();
    let notification_stream = listener.into_stream();
    let key_value_store = KVStorePostgres::new(db_pool.clone());
    tokio::spawn(async move {
        process_notifications(notification_stream, state, key_value_store)
            .await;
    })
}
