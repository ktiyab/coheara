//! ADS: Tauri IPC commands for the App Distribution Server.
//!
//! Commands for the desktop UI to manage the distribution server:
//! - start_distribution: Start serving the mobile companion
//! - stop_distribution: Stop the distribution server
//! - get_distribution_status: Check if server is running + stats
//! - get_install_qr: Get QR code for phone to scan

use std::sync::Arc;

use tauri::State;

use crate::core_state::CoreState;
use crate::distribution::{
    self, DistributionConfig, DistributionStatus, InstallQrCode,
};

/// Start the app distribution server.
///
/// Binds to the local network and begins serving the mobile companion
/// installation page, APK, and PWA. Returns the QR code for the phone.
#[tauri::command]
pub async fn start_distribution(
    state: State<'_, Arc<CoreState>>,
) -> Result<InstallQrCode, String> {
    // Check if profile is unlocked
    if state.is_locked() {
        return Err("Profile must be unlocked to start distribution server".into());
    }

    // Check if already running
    {
        let guard = state.distribution_server.lock().await;
        if guard.is_some() {
            return Err("Distribution server is already running".into());
        }
    }

    // Determine asset paths
    let app_data = crate::config::app_data_dir();
    let pwa_dir = app_data.join("mobile-pwa");
    let apk_path = app_data.join("mobile-apk").join("coheara.apk");

    let config = DistributionConfig {
        port: 0, // Ephemeral port
        rate_limit_per_min: 60,
        pwa_dir: if pwa_dir.join("index.html").exists() {
            Some(pwa_dir)
        } else {
            None
        },
        apk_path: if apk_path.exists() {
            Some(apk_path)
        } else {
            None
        },
    };

    let server = distribution::start_distribution_server(config)
        .await
        .map_err(|e| format!("Failed to start distribution server: {e}"))?;

    let qr = server.qr_code.clone();

    // Store server handle
    let mut guard = state.distribution_server.lock().await;
    *guard = Some(server);

    // Audit log
    state.log_access(
        crate::core_state::AccessSource::DesktopUi,
        "start_distribution",
        "distribution_server",
    );

    tracing::info!(url = %qr.url, "Distribution server started via IPC");

    Ok(qr)
}

/// Stop the app distribution server.
#[tauri::command]
pub async fn stop_distribution(
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let mut guard = state.distribution_server.lock().await;
    match guard.as_mut() {
        Some(server) => {
            server.shutdown();
            *guard = None;

            state.log_access(
                crate::core_state::AccessSource::DesktopUi,
                "stop_distribution",
                "distribution_server",
            );

            tracing::info!("Distribution server stopped via IPC");
            Ok(())
        }
        None => Err("Distribution server is not running".into()),
    }
}

/// Get the current distribution server status.
///
/// Returns `None` if the server is not running.
#[tauri::command]
pub async fn get_distribution_status(
    state: State<'_, Arc<CoreState>>,
) -> Result<Option<DistributionStatus>, String> {
    let guard = state.distribution_server.lock().await;
    match guard.as_ref() {
        Some(server) => {
            let app_data = crate::config::app_data_dir();
            let has_apk = app_data
                .join("mobile-apk")
                .join("coheara.apk")
                .exists();
            let has_pwa = app_data
                .join("mobile-pwa")
                .join("index.html")
                .exists();

            Ok(Some(server.status(has_apk, has_pwa).await))
        }
        None => Ok(None),
    }
}

/// Get the QR code for installing the mobile companion.
///
/// Returns the QR code SVG from the running distribution server.
#[tauri::command]
pub async fn get_install_qr(
    state: State<'_, Arc<CoreState>>,
) -> Result<InstallQrCode, String> {
    let guard = state.distribution_server.lock().await;
    match guard.as_ref() {
        Some(server) => Ok(server.qr_code.clone()),
        None => Err("Distribution server is not running. Start it first.".into()),
    }
}
