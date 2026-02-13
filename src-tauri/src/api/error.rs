//! M0-01: API error types with structured JSON responses.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use crate::core_state::CoreError;

/// Structured error response body for mobile clients.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: &'static str,
    pub message: String,
}

/// API-level errors with HTTP status mapping.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Authentication required")]
    Unauthorized,
    #[error("Token expired")]
    TokenExpired,
    #[error("Rate limit exceeded")]
    RateLimited { retry_after: u64 },
    #[error("Profile not active")]
    NoActiveProfile,
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Invalid request: {0}")]
    BadRequest(String),
    #[error("Nonce invalid or expired")]
    NonceInvalid,
    #[error("Pairing request denied by desktop user")]
    PairingDenied,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            ApiError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "AUTH_REQUIRED",
                "Authentication required".to_string(),
            ),
            ApiError::TokenExpired => (
                StatusCode::UNAUTHORIZED,
                "TOKEN_EXPIRED",
                "Token expired, re-authenticate".to_string(),
            ),
            ApiError::RateLimited { retry_after } => (
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMITED",
                format!("Rate limit exceeded. Retry after {retry_after}s"),
            ),
            ApiError::NoActiveProfile => (
                StatusCode::SERVICE_UNAVAILABLE,
                "PROFILE_LOCKED",
                "No active profile on desktop".to_string(),
            ),
            ApiError::NotFound(detail) => (
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                detail.clone(),
            ),
            ApiError::Internal(detail) => {
                tracing::error!(detail, "API internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL",
                    "An internal error occurred".to_string(),
                )
            }
            ApiError::BadRequest(detail) => (
                StatusCode::BAD_REQUEST,
                "BAD_REQUEST",
                detail.clone(),
            ),
            ApiError::NonceInvalid => (
                StatusCode::BAD_REQUEST,
                "NONCE_INVALID",
                "Nonce invalid or expired".to_string(),
            ),
            ApiError::PairingDenied => (
                StatusCode::FORBIDDEN,
                "PAIRING_DENIED",
                "Desktop user denied the pairing request".to_string(),
            ),
        };

        let body = ErrorBody {
            error: ErrorDetail { code, message },
        };

        let mut response = (status, Json(body)).into_response();
        // Add retry-after header for rate limited responses
        if let ApiError::RateLimited { retry_after } = &self {
            if let Ok(val) = axum::http::HeaderValue::from_str(&retry_after.to_string()) {
                response.headers_mut().insert("Retry-After", val);
            }
        }
        response
    }
}

impl From<CoreError> for ApiError {
    fn from(err: CoreError) -> Self {
        match err {
            CoreError::NoActiveSession => ApiError::NoActiveProfile,
            CoreError::LockPoisoned => ApiError::Internal("lock poisoned".into()),
            CoreError::Database(e) => ApiError::Internal(e.to_string()),
        }
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(err: rusqlite::Error) -> Self {
        ApiError::Internal(err.to_string())
    }
}

impl From<crate::db::DatabaseError> for ApiError {
    fn from(err: crate::db::DatabaseError) -> Self {
        ApiError::Internal(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[tokio::test]
    async fn unauthorized_returns_401() {
        let response = ApiError::Unauthorized.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "AUTH_REQUIRED");
    }

    #[tokio::test]
    async fn rate_limited_returns_429_with_retry_after() {
        let response = ApiError::RateLimited { retry_after: 60 }.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers().get("Retry-After").unwrap(), "60");
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "RATE_LIMITED");
    }

    #[tokio::test]
    async fn no_active_profile_returns_503() {
        let response = ApiError::NoActiveProfile.into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "PROFILE_LOCKED");
    }

    #[tokio::test]
    async fn not_found_returns_404() {
        let response = ApiError::NotFound("Medication not found".into()).into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn bad_request_returns_400() {
        let response = ApiError::BadRequest("Invalid ID format".into()).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn internal_returns_500() {
        let response = ApiError::Internal("something broke".into()).into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        // Internal errors hide details from client
        assert_eq!(json["error"]["message"], "An internal error occurred");
    }

    #[tokio::test]
    async fn nonce_invalid_returns_400() {
        let response = ApiError::NonceInvalid.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "NONCE_INVALID");
    }

    #[tokio::test]
    async fn core_error_no_session_maps_to_no_profile() {
        let api_err: ApiError = CoreError::NoActiveSession.into();
        let response = api_err.into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
