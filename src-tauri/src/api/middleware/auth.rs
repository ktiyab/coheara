//! M0-01: Bearer token authentication middleware.
//!
//! Extracts `Authorization: Bearer <token>`, validates against
//! DeviceRegistry, rotates the token, and injects `DeviceContext`
//! into request extensions for downstream handlers.

use axum::http::{HeaderValue, Request};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::api::error::ApiError;
use crate::api::types::{ApiContext, DeviceContext};

/// Require a valid bearer token from a paired mobile device.
///
/// Accesses `ApiContext` from request extensions (injected by Extension layer).
/// On success: injects `DeviceContext`, adds `X-New-Token` and `Cache-Control` headers.
pub async fn require_auth(
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    match require_auth_inner(req, next).await {
        Ok(resp) => resp,
        Err(err) => err.into_response(),
    }
}

async fn require_auth_inner(
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let ctx: ApiContext = req
        .extensions()
        .get::<ApiContext>()
        .cloned()
        .ok_or(ApiError::Internal("missing API context".into()))?;

    // 1. Extract bearer token
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(ApiError::Unauthorized)?
        .to_string();

    // 2. Validate + rotate via DeviceRegistry
    let (device_id, device_name, new_token) = {
        let mut devices = ctx
            .core
            .write_devices()
            .map_err(|_| ApiError::Internal("device lock".into()))?;

        devices
            .validate_and_rotate(&token)
            .ok_or(ApiError::Unauthorized)?
    }; // RwLockWriteGuard dropped here, before any .await

    // 3. Inject device context for downstream handlers
    req.extensions_mut().insert(DeviceContext {
        device_id,
        device_name,
    });

    // 4. Process request
    let mut response = next.run(req).await;

    // 5. Include rotated token + cache control in response
    if let Ok(val) = HeaderValue::from_str(&new_token) {
        response.headers_mut().insert("X-New-Token", val);
    }
    response
        .headers_mut()
        .insert("Cache-Control", HeaderValue::from_static("no-store"));

    Ok(response)
}
