//! M0-02/M0-03: Authentication endpoints for device pairing and WebSocket tickets.
//!
//! `POST /api/auth/pair` — Unprotected: phone calls after QR scan
//! `POST /api/auth/ws-ticket` — Protected: phone requests one-time WS upgrade ticket

use axum::extract::State;
use axum::{Extension, Json};
use serde::Serialize;

use crate::api::error::ApiError;
use crate::api::types::{ApiContext, DeviceContext};
use crate::pairing::{self, PairRequest, PairResponse};

/// `POST /api/auth/pair` — Phone submits pairing request after QR scan.
///
/// This endpoint:
/// 1. Validates the pairing token (one-time, 5-min expiry)
/// 2. Stores the phone's public key and device info
/// 3. Waits for the desktop user to approve (long-poll, up to 60s)
/// 4. On approval: performs ECDH, registers device, returns session token
/// 5. On denial: returns 403
pub async fn pair(
    State(ctx): State<ApiContext>,
    Json(request): Json<PairRequest>,
) -> Result<Json<PairResponse>, ApiError> {
    // Step 0: Pairing-specific rate limit — 5 req/min (RS-M002-06)
    {
        let mut limiter = ctx
            .pairing_limiter
            .lock()
            .map_err(|_| ApiError::Internal("pairing limiter lock".into()))?;
        limiter
            .check()
            .map_err(|retry_after| ApiError::RateLimited { retry_after })?;
    }

    // Step 1: Submit the pairing request (validates token, stores phone info)
    let approval_rx = {
        let mut pairing = ctx
            .core
            .lock_pairing()
            .map_err(|_| ApiError::Internal("pairing lock".into()))?;
        pairing
            .submit_pair_request(&request)
            .map_err(pairing_error_to_api)?
    };

    // Step 2: Wait for desktop user approval (long-poll with timeout)
    let approved = tokio::time::timeout(pairing::approval_timeout(), approval_rx)
        .await
        .map_err(|_| ApiError::BadRequest("Approval timed out".into()))?
        .map_err(|_| ApiError::Internal("approval channel closed".into()))?;

    if !approved {
        return Err(ApiError::PairingDenied);
    }

    // Step 3: Complete the pairing (ECDH key exchange, generate session token)
    // Desktop already called signal_approval() which unblocked this handler.
    // Now we perform the cryptographic handshake and get the session data.
    let approved_data = {
        let mut pairing = ctx
            .core
            .lock_pairing()
            .map_err(|_| ApiError::Internal("pairing lock".into()))?;
        pairing.complete_pairing().map_err(pairing_error_to_api)?
    };

    // Step 4: Get owner profile info for device registration
    let (owner_profile_id, profile_name) = {
        let guard = ctx.core.read_session().map_err(ApiError::from)?;
        let session = guard.as_ref().ok_or(ApiError::NoActiveProfile)?;
        (session.profile_id.to_string(), session.profile_name.clone())
    };

    // Step 5: Register device in DeviceManager
    let device_id = uuid::Uuid::new_v4().to_string();
    {
        let mut devices = ctx
            .core
            .write_devices()
            .map_err(|_| ApiError::Internal("device lock".into()))?;
        devices
            .register_device(
                device_id.clone(),
                approved_data.device_name.clone(),
                approved_data.device_model.clone(),
                owner_profile_id.clone(),
                approved_data.token_hash,
            )
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    }

    // Step 6: Persist to per-profile database — rollback DeviceManager if DB fails (RS-M002-05)
    let conn = ctx
        .core
        .open_db()
        .map_err(|e| ApiError::Internal(format!("db open: {e}")))?;

    if let Err(e) = pairing::db_store_paired_device(
        &conn,
        &device_id,
        &approved_data.device_name,
        &approved_data.device_model,
        &approved_data.phone_public_key,
    ) {
        // Rollback: remove the in-memory registration
        if let Ok(mut devices) = ctx.core.write_devices() {
            devices.remove_device(&device_id);
        }
        return Err(ApiError::Internal(format!("db persist device: {e}")));
    }

    if let Err(e) = pairing::db_store_session(&conn, &device_id, &approved_data.token_hash) {
        // Rollback: remove both in-memory and DB device record
        if let Ok(mut devices) = ctx.core.write_devices() {
            devices.remove_device(&device_id);
        }
        // Best-effort cleanup of the device row we just inserted
        let _ = conn.execute("DELETE FROM paired_devices WHERE device_id = ?1", [&device_id]);
        return Err(ApiError::Internal(format!("db persist session: {e}")));
    }

    // Step 7: MP-01 — Dual-write to global device_registry + auto-grant managed profiles
    let accessible_profiles = register_device_globally(
        &ctx,
        &device_id,
        &approved_data.device_name,
        &approved_data.device_model,
        &approved_data.phone_public_key,
        &owner_profile_id,
        &profile_name,
    );

    // Step 8: Log the pairing event
    ctx.core.log_access(
        crate::core_state::AccessSource::DesktopUi,
        "pair_device",
        &format!("device:{device_id} profiles:{}", accessible_profiles.len()),
    );

    // Step 9: Encrypt cache key for transport
    let cache_key_b64 =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, approved_data.cache_key);

    Ok(Json(PairResponse {
        session_token: approved_data.session_token,
        cache_key_encrypted: cache_key_b64,
        profile_name,
        accessible_profiles,
    }))
}

// ═══════════════════════════════════════════════════════════
// M0-03: WebSocket ticket endpoint
// ═══════════════════════════════════════════════════════════

/// Response for `POST /api/auth/ws-ticket`.
#[derive(Serialize)]
pub struct WsTicketResponse {
    pub ticket: String,
    pub expires_in: u32,
}

/// `POST /api/auth/ws-ticket` — Generate one-time WebSocket upgrade ticket.
///
/// Requires bearer auth. Returns a ticket valid for 30 seconds.
/// The phone uses this ticket in the WS upgrade query param instead
/// of exposing the session token in a URL.
pub async fn ws_ticket(
    State(ctx): State<ApiContext>,
    Extension(device): Extension<DeviceContext>,
) -> Result<Json<WsTicketResponse>, ApiError> {
    let ticket = {
        let mut tickets = ctx
            .ws_tickets
            .lock()
            .map_err(|_| ApiError::Internal("ticket lock".into()))?;
        tickets.issue(device.device_id, device.device_name)
    };

    Ok(Json(WsTicketResponse {
        ticket,
        expires_in: 30,
    }))
}

/// MP-01: Register device in global app.db and auto-grant access to managed profiles.
///
/// Best-effort: failures here don't block pairing (auth middleware has a fallback
/// to the active session). Returns the list of accessible profiles for the response.
fn register_device_globally(
    ctx: &ApiContext,
    device_id: &str,
    device_name: &str,
    device_model: &str,
    public_key: &[u8; 32],
    owner_profile_id: &str,
    owner_profile_name: &str,
) -> Vec<pairing::AccessibleProfile> {
    use crate::db::repository::device_registry;

    // Start with the owner's own profile
    let mut accessible = vec![pairing::AccessibleProfile {
        profile_id: owner_profile_id.to_string(),
        profile_name: owner_profile_name.to_string(),
        relationship: "own".to_string(),
        color_index: None,
    }];

    // Try to open app.db and register globally
    let app_conn = match ctx.core.open_app_db() {
        Ok(conn) => conn,
        Err(e) => {
            tracing::warn!("MP-01: Could not open app.db for global registration: {e}");
            return accessible;
        }
    };

    // Insert into global device_registry
    let row = device_registry::DeviceRegistryRow {
        device_id: device_id.to_string(),
        device_name: device_name.to_string(),
        device_model: device_model.to_string(),
        owner_profile_id: owner_profile_id.to_string(),
        public_key: public_key.to_vec(),
        paired_at: chrono::Utc::now().to_rfc3339(),
        last_seen: chrono::Utc::now().to_rfc3339(),
        is_revoked: false,
    };
    if let Err(e) = device_registry::insert_device(&app_conn, &row) {
        tracing::warn!("MP-01: Failed to insert into global device_registry: {e}");
        return accessible;
    }

    // Grant access to owner's own profile
    if let Err(e) = device_registry::grant_device_profile_access(
        &app_conn,
        device_id,
        owner_profile_id,
        "full",
    ) {
        tracing::warn!("MP-01: Failed to grant owner profile access: {e}");
    }

    // Find managed profiles and auto-grant access
    let profiles = crate::crypto::profile::list_profiles(&ctx.core.profiles_dir)
        .unwrap_or_default();
    for profile in &profiles {
        // Set color_index for the owner profile
        if profile.id.to_string() == owner_profile_id {
            if let Some(ap) = accessible.first_mut() {
                ap.color_index = profile.color_index;
            }
        }

        // Auto-grant access to profiles managed by the owner
        if let Some(ref managed_by) = profile.managed_by {
            if managed_by == owner_profile_name {
                let profile_id_str = profile.id.to_string();
                if let Err(e) = device_registry::grant_device_profile_access(
                    &app_conn,
                    device_id,
                    &profile_id_str,
                    "full",
                ) {
                    tracing::warn!("MP-01: Failed to grant managed profile access: {e}");
                    continue;
                }
                accessible.push(pairing::AccessibleProfile {
                    profile_id: profile_id_str,
                    profile_name: profile.name.clone(),
                    relationship: "managed".to_string(),
                    color_index: profile.color_index,
                });
            }
        }
    }

    accessible
}

/// Map pairing errors to API errors.
fn pairing_error_to_api(err: pairing::PairingError) -> ApiError {
    match err {
        pairing::PairingError::NoPairingActive => {
            ApiError::BadRequest("No active pairing session".into())
        }
        pairing::PairingError::TokenExpired => ApiError::BadRequest("Pairing token expired".into()),
        pairing::PairingError::TokenInvalid => ApiError::Unauthorized,
        pairing::PairingError::TokenConsumed => {
            ApiError::BadRequest("Pairing token already used".into())
        }
        pairing::PairingError::Denied => ApiError::PairingDenied,
        pairing::PairingError::ApprovalTimeout => {
            ApiError::BadRequest("Approval timed out".into())
        }
        pairing::PairingError::InvalidPublicKey => {
            ApiError::BadRequest("Invalid public key".into())
        }
        pairing::PairingError::MaxDevices => {
            ApiError::BadRequest("Maximum paired devices reached".into())
        }
        other => ApiError::Internal(other.to_string()),
    }
}
