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
use crate::{
    env::ENV_CONFIG,
    kv_store::{KvStore, KVStorePostgres}
};
use crate::caching::CacheKey;
use super::{State, StateExtension};

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

    async fn load_from_db(&self, kv_store: &impl KvStore) {

    }

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
        Some(cached_value) => (headers, Json(cached_value).into_response()).into_response(),
    }
}


lazy_static! {
    static ref SIX_SECONDS: Duration = Duration::seconds(6);
    static ref TWO_MINUTES: Duration = Duration::seconds(120);
}