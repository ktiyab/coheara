//! M0-01 + MP-01: Bearer token authentication middleware with profile-scoped access.
//!
//! Extracts `Authorization: Bearer <token>`, validates against
//! DeviceRegistry, rotates the token, resolves profile access via
//! the 4-rule authorization cascade, and injects `DeviceContext`
//! into request extensions for downstream handlers.

use axum::http::{HeaderValue, Request};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use uuid::Uuid;

use crate::api::error::ApiError;
use crate::api::types::{ApiContext, DeviceContext};
use crate::authorization::{self, AccessLevel};

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

    // 0. Extract source identifier for lockout tracking (RS-M0-01-001)
    let lockout_source = req
        .headers()
        .get("X-Device-Id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("anonymous")
        .to_string();

    // 0b. Check if source is locked out
    {
        let mut lockout = ctx
            .auth_lockout
            .lock()
            .map_err(|_| ApiError::Internal("lockout lock".into()))?;
        if lockout.is_locked(&lockout_source) {
            return Err(ApiError::Unauthorized);
        }
    }

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

        match devices.validate_and_rotate(&token) {
            Some(result) => result,
            None => {
                // Record auth failure for lockout tracking
                if let Ok(mut lockout) = ctx.auth_lockout.lock() {
                    lockout.record_failure(&lockout_source);
                }
                return Err(ApiError::Unauthorized);
            }
        }
    }; // RwLockWriteGuard dropped here, before any .await

    // 2b. Clear lockout on successful auth
    if let Ok(mut lockout) = ctx.auth_lockout.lock() {
        lockout.clear(&lockout_source);
    }

    // 3. Optionally update device metadata from signature headers (CA-01)
    let header_name = req
        .headers()
        .get("X-Device-Name")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let header_model = req
        .headers()
        .get("X-Device-Model")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    if header_name.is_some() || header_model.is_some() {
        if let Ok(mut devices) = ctx.core.write_devices() {
            devices.update_device_metadata(
                &device_id,
                header_name.as_deref(),
                header_model.as_deref(),
            );
        }
    }

    let display_name = header_name.unwrap_or(device_name);

    // 4. MP-01: Resolve owner_profile_id
    //    Try app.db device_registry first, fall back to active session profile.
    let owner_profile_id = resolve_owner_profile_id(&ctx, &device_id);

    // 5. MP-01: Extract target profile from X-Profile-Id header (default: owner)
    let target_profile_id = req
        .headers()
        .get("X-Profile-Id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| Uuid::parse_str(v).ok())
        .unwrap_or(owner_profile_id);

    // 6. MP-01: Authorization check (4-rule cascade)
    let access_level = if owner_profile_id == target_profile_id {
        // Own profile — always full access (fast path, no DB lookup needed)
        AccessLevel::Full
    } else {
        // Cross-profile access — run authorization cascade
        match check_cross_profile_access(&ctx, &owner_profile_id, &target_profile_id, &device_id) {
            Ok(decision) => {
                if !decision.allowed {
                    return Err(ApiError::Forbidden);
                }
                decision.level
            }
            Err(_) => {
                // Authorization check failed — deny by default
                return Err(ApiError::Forbidden);
            }
        }
    };

    // 7. Inject expanded device context for downstream handlers
    req.extensions_mut().insert(DeviceContext {
        device_id,
        device_name: display_name,
        owner_profile_id,
        target_profile_id,
        access_level,
    });

    // 8. Process request
    let mut response = next.run(req).await;

    // 9. Include rotated token + cache control in response
    if let Ok(val) = HeaderValue::from_str(&new_token) {
        response.headers_mut().insert("X-New-Token", val);
    }
    response
        .headers_mut()
        .insert("Cache-Control", HeaderValue::from_static("no-store"));

    Ok(response)
}

/// Resolve the owner profile ID for a device.
///
/// Tries the global device_registry (app.db) first. If the device
/// isn't registered globally yet (pre-migration), falls back to the
/// active desktop profile. This ensures backward compatibility during
/// the CHUNK 5 migration.
fn resolve_owner_profile_id(ctx: &ApiContext, device_id: &str) -> Uuid {
    // Try app.db device_registry
    if let Ok(app_conn) = ctx.core.open_app_db() {
        if let Ok(Some(device)) =
            crate::db::repository::device_registry::get_device(&app_conn, device_id)
        {
            if let Ok(uuid) = Uuid::parse_str(&device.owner_profile_id) {
                return uuid;
            }
        }
    }

    // Fallback: active session's profile_id
    ctx.core
        .read_session()
        .ok()
        .and_then(|guard| guard.as_ref().map(|s| s.profile_id))
        .unwrap_or_else(Uuid::nil)
}

/// Check cross-profile access using the authorization cascade.
fn check_cross_profile_access(
    ctx: &ApiContext,
    owner_profile_id: &Uuid,
    target_profile_id: &Uuid,
    device_id: &str,
) -> Result<authorization::AccessDecision, ApiError> {
    let app_conn = ctx
        .core
        .open_app_db()
        .map_err(|e| ApiError::Internal(format!("app db: {e}")))?;

    authorization::check_profile_access(
        &app_conn,
        &ctx.core.profiles_dir,
        owner_profile_id,
        target_profile_id,
        device_id,
    )
    .map_err(|e| ApiError::Internal(format!("authz: {e}")))
}
