//! ADS: Tauri IPC commands for the App Distribution Server.
//!
//! Commands for the desktop UI to manage the distribution server:
//! - start_distribution: Start serving the mobile companion
//! - stop_distribution: Stop the distribution server
//! - get_distribution_status: Check if server is running + stats
//! - get_install_qr: Get QR code for phone to scan

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::core_state::CoreState;
use crate::distribution::{
    self, DistributionConfig, DistributionStatus, InstallQrCode,
};

/// Resolve mobile PWA directory: bundled resources first, then app data fallback.
fn resolve_pwa_dir(app: &AppHandle) -> Option<PathBuf> {
    // 1. Bundled resources (production builds)
    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled = resource_dir.join("resources").join("mobile-pwa");
        if bundled.join("index.html").exists() {
            return Some(bundled);
        }
    }

    // 2. App data directory (user-placed or dev mode)
    let app_data = crate::config::app_data_dir();
    let user_pwa = app_data.join("mobile-pwa");
    if user_pwa.join("index.html").exists() {
        return Some(user_pwa);
    }

    None
}

/// Resolve APK path: bundled resources first, then app data fallback.
fn resolve_apk_path(app: &AppHandle) -> Option<PathBuf> {
    // 1. Bundled resources (production builds — APK shipped inside desktop installer)
    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled = resource_dir.join("resources").join("mobile-apk").join("coheara.apk");
        if bundled.exists() {
            return Some(bundled);
        }
    }

    // 2. App data directory (user-placed or dev mode)
    let apk = crate::config::app_data_dir()
        .join("mobile-apk")
        .join("coheara.apk");
    if apk.exists() { Some(apk) } else { None }
}

/// Start the app distribution server.
///
/// Binds to the local network and begins serving the mobile companion
/// installation page, APK, and PWA. Returns the QR code for the phone.
#[tauri::command]
pub async fn start_distribution(
    state: State<'_, Arc<CoreState>>,
    app: AppHandle,
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

    // SEC-HTTPS-01: Load CA certificate (public part only) for trust onboarding.
    // The distribution server serves this over HTTP so phones can install it
    // before connecting to the HTTPS mobile API server.
    let ca_cert_der = {
        let conn = state.open_db().map_err(|e| e.to_string())?;
        let guard = state.read_session().map_err(|e| e.to_string())?;
        let session = guard.as_ref().ok_or("No active session")?;
        match crate::local_ca::load_ca(&conn, session.key_bytes()) {
            Ok(ca) => Some(ca.cert_der),
            Err(crate::local_ca::LocalCaError::NotFound) => {
                tracing::info!("No CA cert yet — CA trust endpoints will be disabled");
                None
            }
            Err(e) => {
                tracing::warn!("Failed to load CA cert: {e} — CA trust endpoints disabled");
                None
            }
        }
    };

    // Resolve asset paths: bundled resources first, then user data fallback
    let config = DistributionConfig {
        port: 0, // Ephemeral port
        rate_limit_per_min: 60,
        pwa_dir: resolve_pwa_dir(&app),
        apk_path: resolve_apk_path(&app),
        core_state: Some(state.inner().clone()),
        ca_cert_der,
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
    app: AppHandle,
) -> Result<Option<DistributionStatus>, String> {
    let guard = state.distribution_server.lock().await;
    match guard.as_ref() {
        Some(server) => {
            let has_apk = resolve_apk_path(&app).is_some();
            let has_pwa = resolve_pwa_dir(&app).is_some();

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
