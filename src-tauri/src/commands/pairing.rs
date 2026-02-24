//! M0-02 + SEC-HTTPS-01: Desktop IPC commands for device pairing.
//!
//! Commands for the desktop UI to manage the pairing flow:
//! - start_pairing: Generate QR code for phone to scan (reads cert from running API server)
//! - cancel_pairing: Cancel an active pairing session
//! - get_pending_approval: Check if a phone is waiting for approval
//! - approve_pairing: Allow the pending device
//! - deny_pairing: Reject the pending device

use std::sync::Arc;

use tauri::State;

use crate::core_state::CoreState;
use crate::pairing::{PairingStartResponse, PendingApproval};

/// Start a new pairing session. Generates QR code for the phone to scan.
///
/// SEC-HTTPS-01: Reads the TLS certificate fingerprint and port from the
/// running mobile API server (rather than generating a separate cert).
/// This ensures the fingerprint in the QR code matches the actual server cert.
///
/// Async because it reads from the tokio Mutex holding the API server state.
#[tauri::command]
pub async fn start_pairing(
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

    // SEC-HTTPS-01: Read cert fingerprint and port from running mobile API server.
    // The QR code must contain the fingerprint of the actual server cert for cert pinning.
    let (cert_fingerprint, api_port) = {
        let guard = state.api_server.lock().await;
        let server = guard.as_ref().ok_or(
            "Mobile API server must be running before pairing. Start it first.",
        )?;
        let fingerprint = server.cert_fingerprint.clone().ok_or(
            "No TLS certificate available. Restart the mobile API server.",
        )?;
        (fingerprint, server.session.port)
    }; // tokio Mutex guard dropped before acquiring sync locks

    // Build server URL with actual HTTPS port
    let local_ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    let server_url = format!("https://{local_ip}:{api_port}");

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
/// Signals approval to the phone's waiting HTTP handler. The actual
/// ECDH key exchange and device registration happens in the HTTP
/// endpoint (`POST /api/auth/pair`) after the phone receives this signal.
#[tauri::command]
pub fn approve_pairing(state: State<'_, Arc<CoreState>>) -> Result<(), String> {
    let mut pairing = state.lock_pairing().map_err(|e| e.to_string())?;
    pairing.signal_approval().map_err(|e| e.to_string())?;

    // Audit log
    state.log_access(
        crate::core_state::AccessSource::DesktopUi,
        "approve_pairing",
        "pairing_signal",
    );

    tracing::info!("Desktop user approved pairing — phone handler will complete ECDH");

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
