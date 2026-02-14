//! E2E-B06: Tauri IPC commands for the Mobile API server.
//!
//! Commands for the desktop UI to manage the mobile API server:
//! - start_mobile_api: Start the axum HTTP server serving M0-01 endpoints
//! - stop_mobile_api: Stop the mobile API server
//! - get_mobile_api_status: Check if server is running + session info

use std::sync::Arc;

use tauri::State;

use crate::api::server;
use crate::api::{MobileApiSession, MobileApiStatus};
use crate::core_state::CoreState;

/// Start the mobile API server.
///
/// Binds to the local network IP on an ephemeral port and begins
/// serving all M0-01 API endpoints (REST + WebSocket). Mobile
/// devices connect after pairing via QR code.
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

    let core = Arc::clone(&state);
    let api_server = server::start_mobile_api_server(core)
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

    tracing::info!(addr = %session.server_addr, "Mobile API server started via IPC");

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
