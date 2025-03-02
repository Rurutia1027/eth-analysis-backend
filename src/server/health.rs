use crate::health::{HealthCheckable, HealthStatus};
use axum::response::IntoResponse;
use chrono::{DateTime, Duration, Utc};
use std::sync::RwLock;

pub struct ServerHealth {
    last_cache_update: RwLock<Option<DateTime<Utc>>>,
    started_on: DateTime<Utc>,
}
impl ServerHealth {
    pub fn new(started_on: DateTime<Utc>) -> Self {
        Self {
            last_cache_update: RwLock::new(None),
            started_on,
        }
    }

    pub fn set_cache_updated(&self) {
        *self.last_cache_update.write().unwrap() = Some(Utc::now());
    }
}

impl HealthCheckable for ServerHealth {
    // health status: an update is seen in the last five minutes, or it has been <= 5 mins since the server started.
    fn health_status(&self) -> HealthStatus {
        let now = Utc::now();
        let last_update = self
            .last_cache_update
            .read()
            .unwrap()
            .unwrap_or(self.started_on);
        let time_since_last_update = now - last_update;
        if time_since_last_update < Duration::minutes(5) {
            HealthStatus::Healthy(Some(format!(
                "[Health] cache has been updated in last 5 minutes"
            )))
        } else {
            HealthStatus::UnHealthy(Some(format!(
                "[UnHealth] cache has not been updated in {} seconds",
                time_since_last_update.num_seconds()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_health_status() {
        // Given
        let started_on = Utc::now();
        let health = ServerHealth::new(started_on);

        // When
        let status = health.health_status();

        // Then
        match status {
            HealthStatus::Healthy(Some(_)) => {} // Should be healthy as server has just started
            HealthStatus::UnHealthy(Some(_)) => {
                panic!("Server should be healthy at start")
            }
            _ => panic!("Unexpected health status"),
        }
    }

    #[test]
    fn test_health_status_after_cache_update() {
        // Given
        let started_on = Utc::now();
        let mut health = ServerHealth::new(started_on);

        // When: simulate a cache update
        health.set_cache_updated();

        // Then: It should still be healthy
        let status = health.health_status();
        match status {
            HealthStatus::Healthy(Some(_)) => {} // Should be healthy because cache was updated
            HealthStatus::UnHealthy(Some(_)) => {
                panic!("Cache was updated, should be healthy")
            }
            _ => panic!("Unexpected health status"),
        }
    }

    //#[test]
    fn test_health_status_after_timeout() {
        // Given: Simulate server start time + 6 minutes
        let started_on = Utc::now() - Duration::minutes(6);
        let mut health = ServerHealth::new(started_on);

        // When: simulate a cache update more than 6 minutes later (timeout)
        health.set_cache_updated();

        // After waiting 6 minutes
        std::thread::sleep(std::time::Duration::from_secs(360));

        // Then: Health status should be unhealthy
        let status = health.health_status();
        match status {
            HealthStatus::UnHealthy(Some(msg)) => {
                assert!(msg.contains("cache has not been updated"));
            }
            _ => panic!("Expected UnHealthy status after timeout"),
        }
    }

    #[test]
    fn test_health_status_with_no_cache_update_but_healthy_within_5min() {
        // Given: Simulate a server started 4 minutes ago
        let started_on = Utc::now() - Duration::minutes(4);
        let health = ServerHealth::new(started_on);

        // When: no cache update

        // Then: Health status should be healthy as it has been less than 5 minutes since server start
        let status = health.health_status();
        match status {
            HealthStatus::Healthy(Some(_)) => {} // Should be healthy as it is within the 5-minute window
            HealthStatus::UnHealthy(Some(_)) => {
                panic!("Server should be healthy within 5 minutes")
            }
            _ => panic!("Unexpected health status"),
        }
    }

    #[test]
    fn test_health_status_with_no_cache_update_and_unhealthy_beyond_5min() {
        // Given: Simulate a server started 6 minutes ago
        let started_on = Utc::now() - Duration::minutes(6);
        let health = ServerHealth::new(started_on);

        // When: no cache update

        // Then: Health status should be unhealthy
        let status = health.health_status();
        match status {
            HealthStatus::UnHealthy(Some(msg)) => {
                assert!(msg.contains("cache has not been updated"));
            }
            _ => panic!("Expected UnHealthy status beyond 5 minutes"),
        }
    }
}
