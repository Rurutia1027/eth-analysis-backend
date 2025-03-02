use axum::{
    response::{IntoResponse, Response},
    Json,
};
use reqwest::StatusCode;
use serde_json::json;

pub enum HealthStatus {
    Healthy(Option<String>),
    UnHealthy(Option<String>),
}

pub trait HealthCheckable {
    fn health_status(&self) -> HealthStatus;
}

impl IntoResponse for HealthStatus {
    fn into_response(self) -> Response {
        match self {
            HealthStatus::Healthy(message) => {
                let message = message.unwrap_or_else(|| {
                    "eth-analysis module health".to_string()
                });
                let body = json!({ "message": message });
                (StatusCode::OK, Json(body)).into_response()
            }
            HealthStatus::UnHealthy(message) => {
                let message = message.unwrap_or_else(|| {
                    "eth-analysis module unhealthy".to_string()
                });
                let body = json!({ "message": message });
                (StatusCode::SERVICE_UNAVAILABLE, Json(body)).into_response()
            }
        }
    }
}
