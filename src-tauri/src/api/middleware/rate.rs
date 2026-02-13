//! M0-01: Per-device rate limiting middleware.
//!
//! Applies sliding-window rate limits per device:
//! - 100 requests per minute
//! - 1000 requests per hour

use axum::http::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::api::error::ApiError;
use crate::api::types::ApiContext;

/// Extract a rate-limit key from the request.
fn rate_key(req: &Request<axum::body::Body>) -> String {
    req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|token| {
            let prefix: String = token.chars().take(16).collect();
            format!("token:{prefix}")
        })
        .unwrap_or_else(|| "anonymous".to_string())
}

/// Per-device rate limiting. Returns 429 if exceeded.
/// Accesses `ApiContext` from request extensions.
pub async fn limit(
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    match limit_inner(req, next).await {
        Ok(response) => response,
        Err(err) => err.into_response(),
    }
}

async fn limit_inner(
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let ctx: ApiContext = req
        .extensions()
        .get::<ApiContext>()
        .cloned()
        .ok_or(ApiError::Internal("missing API context".into()))?;

    let key = rate_key(&req);

    // MutexGuard is !Send — must drop before .await via block scope
    {
        let mut limiter = ctx
            .rate_limiter
            .lock()
            .map_err(|_| ApiError::Internal("rate limiter lock".into()))?;

        limiter
            .check(&key)
            .map_err(|retry_after| ApiError::RateLimited { retry_after })?;
    }

    Ok(next.run(req).await)
}
