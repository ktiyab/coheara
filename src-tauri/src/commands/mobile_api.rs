//! E2E-B06 + SEC-HTTPS-01: Tauri IPC commands for the Mobile API server.
//!
//! Commands for the desktop UI to manage the mobile API server:
//! - start_mobile_api: Start the HTTPS server with local CA-signed cert
//! - stop_mobile_api: Stop the mobile API server
//! - get_mobile_api_status: Check if server is running + session info

use std::sync::Arc;

use tauri::State;

use crate::api::server;
use crate::api::{MobileApiSession, MobileApiStatus};
use crate::core_state::CoreState;
use crate::local_ca;

/// Start the mobile API server with HTTPS.
///
/// Flow (Home Assistant + Synology pattern):
/// 1. Load or generate the local Certificate Authority
/// 2. Issue a server certificate signed by the CA (SAN = local IP)
/// 3. Start the HTTPS server with the cert bundle
/// 4. Store the server handle (with cert fingerprint for pairing QR)
#[tauri::command]
pub async fn start_mobile_api(
    state: State<'_, Arc<CoreState>>,
) -> Result<MobileApiSession, String> {
    // Check if profile is unlocked
    if state.is_locked() {
        return Err("Profile must be unlocked to start mobile API server".into());
    }

    // Check if already running
    {
        let guard = state.api_server.lock().await;
        if guard.is_some() {
            return Err("Mobile API server is already running".into());
        }
    }

    // SEC-HTTPS-01: Load/generate CA + issue server cert
    let cert_bundle = {
        let local_ip = local_ip_address::local_ip()
            .map_err(|e| format!("Cannot detect local IP: {e}"))?;

        let conn = state.open_db().map_err(|e| e.to_string())?;
        let guard = state.read_session().map_err(|e| e.to_string())?;
        let session = guard.as_ref().ok_or("No active session")?;
        let profile_key = session.key_bytes();

        let ca = local_ca::load_or_generate_ca(&conn, profile_key)
            .map_err(|e| format!("Failed to load/generate CA: {e}"))?;

        local_ca::issue_server_cert(&ca, local_ip)
            .map_err(|e| format!("Failed to issue server certificate: {e}"))?
    };

    let core = Arc::clone(&state);
    let api_server = server::start_mobile_api_server(core, cert_bundle)
        .await
        .map_err(|e| format!("Failed to start mobile API server: {e}"))?;

    let session = api_server.session.clone();

    // Store server handle
    let mut guard = state.api_server.lock().await;
    *guard = Some(api_server);

    // Audit log
    state.log_access(
        crate::core_state::AccessSource::DesktopUi,
        "start_mobile_api",
        "api_server",
    );

    tracing::info!(addr = %session.server_addr, "Mobile API HTTPS server started via IPC");

    Ok(session)
}

/// Stop the mobile API server.
#[tauri::command]
pub async fn stop_mobile_api(
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let mut guard = state.api_server.lock().await;
    match guard.as_mut() {
        Some(server) => {
            server.shutdown();
            *guard = None;

            state.log_access(
                crate::core_state::AccessSource::DesktopUi,
                "stop_mobile_api",
                "api_server",
            );

            tracing::info!("Mobile API server stopped via IPC");
            Ok(())
        }
        None => Err("Mobile API server is not running".into()),
    }
}

/// Get the current mobile API server status.
///
/// Returns whether the server is running and its session metadata.
#[tauri::command]
pub async fn get_mobile_api_status(
    state: State<'_, Arc<CoreState>>,
) -> Result<MobileApiStatus, String> {
    let guard = state.api_server.lock().await;
    match guard.as_ref() {
        Some(server) => Ok(MobileApiStatus {
            running: true,
            session: Some(server.session.clone()),
        }),
        None => Ok(MobileApiStatus {
            running: false,
            session: None,
        }),
    }
}
