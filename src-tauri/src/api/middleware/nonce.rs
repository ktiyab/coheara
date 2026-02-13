//! M0-01: Nonce anti-replay middleware.
//!
//! Requires `X-Request-Nonce` and `X-Request-Timestamp` headers.
//! Rejects if: no nonce, timestamp >30s old, or nonce already seen.

use axum::http::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::api::error::ApiError;
use crate::api::types::ApiContext;

/// Maximum allowed clock skew between client and server (30 seconds).
const MAX_TIMESTAMP_DRIFT_SECS: i64 = 30;

/// Verify request nonce and timestamp for anti-replay protection.
/// Accesses `ApiContext` from request extensions.
pub async fn verify_nonce(
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    match verify_nonce_inner(req, next).await {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

async fn verify_nonce_inner(
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let ctx: ApiContext = req
        .extensions()
        .get::<ApiContext>()
        .cloned()
        .ok_or(ApiError::Internal("missing API context".into()))?;

    let nonce = req
        .headers()
        .get("X-Request-Nonce")
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::NonceInvalid)?
        .to_string();

    let timestamp_str = req
        .headers()
        .get("X-Request-Timestamp")
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::NonceInvalid)?
        .to_string();

    // Validate timestamp freshness
    let ts: i64 = timestamp_str
        .parse()
        .map_err(|_| ApiError::NonceInvalid)?;
    let now = chrono::Utc::now().timestamp();
    if (now - ts).abs() > MAX_TIMESTAMP_DRIFT_SECS {
        return Err(ApiError::NonceInvalid);
    }

    // Check and insert nonce (reject replays)
    // MutexGuard is !Send — must drop before .await via block scope
    {
        let mut cache = ctx
            .nonce_cache
            .lock()
            .map_err(|_| ApiError::Internal("nonce cache lock".into()))?;

        if !cache.check_and_insert(&nonce) {
            return Err(ApiError::NonceInvalid);
        }
    }

    Ok(next.run(req).await)
}
