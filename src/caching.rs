use crate::{
    kv_store::{self, KvStore},
    time_frames::{GrowingTimeFrame, LimitedTimeFrame, TimeFrame},
};
use enum_iterator::Sequence;
use log::debug;
use serde::Serialize;
use serde_json::Value;
use sqlx::{PgExecutor, PgPool};
use std::fmt::Display;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, Sequence)]
pub enum CacheKey {
    AverageEthPrice,
    BaseFeeOverTime,
    BaseFeePerGas,
    BaseFeePerGasBarrier,
    BaseFeePerGasStats,
    BaseFeePerGasStatsTimeFrame(TimeFrame),
    BlobFeePerGasStats,
    BlobFeePerGasStatsTimeFrame(TimeFrame),
    BlockLag,
    BurnRates,
    BurnSums,
    EffectiveBalanceSum,
    EthPrice,
    FlippeningData,
    GaugeRates,
    SupplyParts,
    IssuanceBreakdown,
    IssuanceEstimate,
    SupplyChanges,
    SupplyDashboardAnalysis,
    SupplyOverTime,
    SupplyProjectionInputs,
    SupplySinceMerge,
    TotalDifficultyProgress,
    ValidatorRewards,
}

impl CacheKey {
    pub fn to_db_key(self) -> &'static str {
        use CacheKey::*;
        use GrowingTimeFrame::*;
        use LimitedTimeFrame::*;
        use TimeFrame::*;

        match self {
            AverageEthPrice => "average-eth-price",
            BaseFeeOverTime => "base-fee-over-time",
            BaseFeePerGas => "current-base-fee",
            BaseFeePerGasBarrier => "base-fee-per-gas-barrier",
            BaseFeePerGasStats => "base-fee-per-gas-stats",
            BaseFeePerGasStatsTimeFrame(time_frame) => match time_frame {
                Growing(SinceBurn) => "base-fee-per-gas-stats-since_burn",
                Growing(SinceMerge) => "base-fee-per-gas-stats-since_merge",
                Limited(Minute5) => "base-fee-per-gas-stats-m5",
                Limited(Hour1) => "base-fee-per-gas-stats-h1",
                Limited(Day1) => "base-fee-per-gas-stats-d1",
                Limited(Day7) => "base-fee-per-gas-stats-d7",
                Limited(Day30) => "base-fee-per-gas-stats-d30",
            },
            BlobFeePerGasStats => "blob-fee-per-gas-stats",
            BlobFeePerGasStatsTimeFrame(time_frame) => match time_frame {
                Growing(SinceBurn) => "blob-fee-per-gas-stats-since_burn",
                Growing(SinceMerge) => "blob-fee-per-gas-stats-since_merge",
                Limited(Minute5) => "blob-fee-per-gas-stats-m5",
                Limited(Hour1) => "blob-fee-per-gas-stats-h1",
                Limited(Day1) => "blob-fee-per-gas-stats-d1",
                Limited(Day7) => "blob-fee-per-gas-stats-d7",
                Limited(Day30) => "blob-fee-per-gas-stats-d30",
            },
            BlockLag => "block-lag",
            BurnRates => "burn-rates",
            BurnSums => "burn-sums",
            EffectiveBalanceSum => "effective-balance-sum",
            EthPrice => "eth-price",
            FlippeningData => "flippening-data",
            GaugeRates => "gauge-rates",
            IssuanceBreakdown => "issuance-breakdown",
            IssuanceEstimate => "issuance-estimate",
            SupplyChanges => "supply-changes",
            SupplyDashboardAnalysis => "supply-dashboard-analysis",
            SupplyOverTime => "supply-over-time",
            SupplyParts => "supply-parts",
            SupplyProjectionInputs => "supply-projection-inputs",
            SupplySinceMerge => "supply-since-merge",
            TotalDifficultyProgress => "total-difficulty-progress",
            ValidatorRewards => "validator-rewards",
        }
    }
}

impl Display for CacheKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_db_key())
    }
}

#[derive(Debug, Error)]
pub enum ParseCacheKeyError {
    #[error("failed to parse cache key {0}")]
    UnknownCacheKey(String),
}

impl FromStr for CacheKey {
    type Err = ParseCacheKeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "average-eth-price" => Ok(Self::AverageEthPrice),
            "base-fee-over-time" => Ok(Self::BaseFeeOverTime),
            "current-base-fee" => Ok(Self::BaseFeePerGas),
            "base-fee-per-gas" => Ok(Self::BaseFeePerGas),
            "base-fee-per-gas-barrier" => Ok(Self::BaseFeePerGasBarrier),
            "base-fee-per-gas-stats" => Ok(Self::BaseFeePerGasStats),
            "blob-fee-per-gas-stats" => Ok(Self::BlobFeePerGasStats),
            "block-lag" => Ok(Self::BlockLag),
            "burn-rates" => Ok(Self::BurnRates),
            "burn-sums" => Ok(Self::BurnSums),
            "effective-balance-sum" => Ok(Self::EffectiveBalanceSum),
            "eth-price" => Ok(Self::EthPrice),
            "flippening-data" => Ok(Self::FlippeningData),
            "gauge-rates" => Ok(Self::GaugeRates),
            "issuance-breakdown" => Ok(Self::IssuanceBreakdown),
            "issuance-estimate" => Ok(Self::IssuanceEstimate),
            "supply-changes" => Ok(Self::SupplyChanges),
            "supply-dashboard-analysis" => Ok(Self::SupplyDashboardAnalysis),
            "supply-over-time" => Ok(Self::SupplyOverTime),
            "supply-parts" => Ok(Self::SupplyParts),
            "supply-projection-inputs" => Ok(Self::SupplyProjectionInputs),
            "supply-since-merge" => Ok(Self::SupplySinceMerge),
            "total-difficulty-progress" => Ok(Self::TotalDifficultyProgress),
            "validator-rewards" => Ok(Self::ValidatorRewards),
            unknown_key if unknown_key.starts_with("base-fee-per-gas-stats-") => unknown_key
                .split('-')
                .nth(5)
                .expect(
                    "expect keys which start with 'base-fee-per-gas-stats-' to have a time frame",
                )
                .to_string()
                .parse::<TimeFrame>()
                .map_or(
                    Err(ParseCacheKeyError::UnknownCacheKey(unknown_key.to_string())),
                    |key| Ok(Self::BaseFeePerGasStatsTimeFrame(key)),
                ),
            unknown_key => Err(ParseCacheKeyError::UnknownCacheKey(unknown_key.to_string())),
        }
    }
}

pub async fn publish_cache_update<'a>(
    executor: impl PgExecutor<'a>,
    key: &CacheKey,
) {
    sqlx::query!(
        "
            SELECT pg_notify('cache-update', $1)
        ",
        key.to_db_key()
    )
    .execute(executor)
    .await
    .unwrap();
}

pub async fn get_serialized_caching_value(
    key_value_store: &impl KvStore,
    cache_key: &CacheKey,
) -> Option<Value> {
    key_value_store.get(cache_key.to_db_key()).await
}

pub async fn set_value<'a>(
    executor: impl PgExecutor<'_>,
    cache_key: &CacheKey,
    value: impl Serialize,
) {
    kv_store::set_value(
        executor,
        cache_key.to_db_key(),
        &serde_json::to_value(value).expect("expect value to be serializable"),
    )
    .await;
}

pub async fn update_and_publish(
    db_pool: &PgPool,
    cache_key: &CacheKey,
    value: impl Serialize,
) {
    set_value(db_pool, cache_key, value).await;
    publish_cache_update(db_pool, cache_key).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, env::ENV_CONFIG, kv_store::KVStorePostgres};
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
    struct TestJson {
        name: String,
        age: i32,
    }



    // 	1.	Establish a PostgreSQL database connection and bind it to the Cache.
    // 	2.	Create a listener to monitor the cache-update event channel in PostgreSQL.
    // 	3.	When an update occurs, send an event through the cache-update channel with the EffectiveBalanceSum key as a string.
    // 	4.	Retrieve and parse the event’s payload, then verify if it matches the expected EffectiveBalanceSum key.
    #[tokio::test]
    async fn test_publish_cache_update() {
        let mut connection = db::db::tests::get_test_db_connection().await;
        publish_cache_update(&mut connection, &CacheKey::EffectiveBalanceSum).await;

        let mut listener =
            sqlx::postgres::PgListener::connect(ENV_CONFIG.db_url.as_str())
                .await
                .unwrap();
        listener.listen("cache-update").await.unwrap();
        let notification_future = async { listener.recv().await };

        let notification = notification_future.await.unwrap();
        assert_eq!(
            notification.payload(),
            CacheKey::EffectiveBalanceSum.to_db_key()
        )
    }
}
