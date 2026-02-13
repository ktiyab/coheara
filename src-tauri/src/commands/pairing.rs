//! M0-02: Desktop IPC commands for device pairing.
//!
//! Commands for the desktop UI to manage the pairing flow:
//! - start_pairing: Generate QR code for phone to scan
//! - cancel_pairing: Cancel an active pairing session
//! - get_pending_approval: Check if a phone is waiting for approval
//! - approve_pairing: Allow the pending device
//! - deny_pairing: Reject the pending device

use std::sync::Arc;

use tauri::State;

use crate::core_state::CoreState;
use crate::pairing::{PairingStartResponse, PendingApproval};
use crate::tls_cert;

/// Start a new pairing session. Generates QR code for the phone to scan.
///
/// This will:
/// 1. Generate/load a TLS certificate (for cert pinning)
/// 2. Create an X25519 keypair and pairing token
/// 3. Return QR code SVG + metadata
#[tauri::command]
pub fn start_pairing(
    state: State<'_, Arc<CoreState>>,
) -> Result<PairingStartResponse, String> {
    // Check if profile is unlocked
    if state.is_locked() {
        return Err("Profile must be unlocked to pair a device".into());
    }

    // Check max devices
    {
        let devices = state.read_devices().map_err(|e| e.to_string())?;
        if !devices.can_pair() {
            return Err("Maximum paired devices reached. Unpair a device first.".into());
        }
    }

    // Get TLS cert fingerprint (generate if needed)
    let cert_fingerprint = {
        let conn = state.open_db().map_err(|e| e.to_string())?;
        let guard = state.read_session().map_err(|e| e.to_string())?;
        let session = guard.as_ref().ok_or("No active session")?;
        let cert = tls_cert::load_or_generate(&conn, session.key_bytes())
            .map_err(|e| e.to_string())?;
        cert.fingerprint
    };

    // Get the local IP for the server URL
    let local_ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    let server_url = format!("https://{local_ip}:8443");

    // Start pairing session
    let mut pairing = state.lock_pairing().map_err(|e| e.to_string())?;
    pairing
        .start(server_url, cert_fingerprint)
        .map_err(|e| e.to_string())
}

/// Cancel the active pairing session.
#[tauri::command]
pub fn cancel_pairing(state: State<'_, Arc<CoreState>>) -> Result<(), String> {
    let mut pairing = state.lock_pairing().map_err(|e| e.to_string())?;
    pairing.cancel();
    Ok(())
}

/// Check if there's a pending pairing approval from a phone.
///
/// Returns `Some(PendingApproval)` with device info, or `None`.
#[tauri::command]
pub fn get_pending_approval(
    state: State<'_, Arc<CoreState>>,
) -> Result<Option<PendingApproval>, String> {
    let pairing = state.lock_pairing().map_err(|e| e.to_string())?;
    Ok(pairing.pending_approval())
}

/// Approve the pending pairing request.
///
/// Completes the ECDH key exchange and registers the device.
#[tauri::command]
pub fn approve_pairing(state: State<'_, Arc<CoreState>>) -> Result<(), String> {
    let approved = {
        let mut pairing = state.lock_pairing().map_err(|e| e.to_string())?;
        pairing.approve().map_err(|e| e.to_string())?
    };

    // Register device in DeviceManager
    let device_id = uuid::Uuid::new_v4().to_string();
    {
        let mut devices = state.write_devices().map_err(|e| e.to_string())?;
        devices
            .register_device(
                device_id.clone(),
                approved.device_name.clone(),
                approved.device_model.clone(),
                approved.token_hash,
            )
            .map_err(|e| e.to_string())?;
    }

    // Persist to database
    if let Ok(conn) = state.open_db() {
        let _ = crate::pairing::db_store_paired_device(
            &conn,
            &device_id,
            &approved.device_name,
            &approved.device_model,
            &approved.phone_public_key,
        );
        let _ = crate::pairing::db_store_session(&conn, &device_id, &approved.token_hash);
    }

    // Audit log
    state.log_access(
        crate::core_state::AccessSource::DesktopUi,
        "approve_pairing",
        &format!("device:{device_id}"),
    );

    tracing::info!(
        device_id,
        device_name = approved.device_name,
        "Device pairing approved"
    );

    Ok(())
}

/// Deny the pending pairing request.
#[tauri::command]
pub fn deny_pairing(state: State<'_, Arc<CoreState>>) -> Result<(), String> {
    let mut pairing = state.lock_pairing().map_err(|e| e.to_string())?;
    pairing.deny();

    tracing::info!("Device pairing denied by user");
    Ok(())
}
