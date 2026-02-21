//! L4-03: WiFi Transfer IPC commands.
//!
//! Exposes the WiFi transfer server to the Svelte frontend via Tauri commands.
//! Files received are staged in the profile directory, then processed through
//! the L1-01 document import pipeline on demand.

use std::path::Path;
use std::sync::Arc;

use tauri::State;

use crate::core_state::CoreState;
use crate::db::sqlite::open_database;
use crate::pipeline::import::importer::import_file;
use crate::pipeline::import::staging::decrypt_staging_to_temp;
use crate::wifi_transfer::{
    generate_qr_code, start_transfer_server, QrCodeData, TransferConfig,
    TransferStatusResponse,
};

/// Derive the WiFi staging directory from the profile's database path.
/// Profile structure: profiles/{uuid}/database/profile.db
/// Staging dir: profiles/{uuid}/wifi_staging/
fn staging_dir_from_db_path(db_path: &Path) -> Result<std::path::PathBuf, String> {
    db_path
        .parent() // database/
        .and_then(|p| p.parent()) // profile dir
        .map(|p| p.join("wifi_staging"))
        .ok_or_else(|| "Invalid profile path structure".to_string())
}

/// Start the WiFi transfer server. Returns QR code data for display.
#[tauri::command]
pub async fn start_wifi_transfer(
    state: State<'_, Arc<CoreState>>,
) -> Result<QrCodeData, String> {
    // Verify active session and get encryption key
    let (staging_dir, encryption_key) = {
        let guard = state.read_session().map_err(|e| e.to_string())?;
        let session = guard
            .as_ref()
            .ok_or("No active profile session")?;
        (staging_dir_from_db_path(session.db_path())?, *session.key_bytes())
    };

    // Check if server already running
    {
        let server_guard = state.transfer_server.lock().await;
        if server_guard.is_some() {
            return Err("Transfer server already running".into());
        }
    }

    let config = TransferConfig::default();
    // SEC-02-G04: Pass encryption key so staging files are encrypted at rest
    let server = start_transfer_server(staging_dir, config, encryption_key).await?;

    // Generate QR code
    let qr_svg = generate_qr_code(&server.session.url)?;
    let qr_data = QrCodeData {
        url: server.session.url.clone(),
        pin: server.session.pin.clone(),
        svg: qr_svg,
    };

    // Store server handle
    *state.transfer_server.lock().await = Some(server);

    state.update_activity();
    Ok(qr_data)
}

/// Stop the WiFi transfer server.
#[tauri::command]
pub async fn stop_wifi_transfer(
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let mut server_opt = state.transfer_server.lock().await;
    if let Some(server) = server_opt.as_mut() {
        server.shutdown();
    }
    *server_opt = None;
    state.update_activity();
    Ok(())
}

/// Get current transfer status (session info + received files).
/// Returns None if no server is running.
#[tauri::command]
pub async fn get_transfer_status(
    state: State<'_, Arc<CoreState>>,
) -> Result<Option<TransferStatusResponse>, String> {
    let server_opt = state.transfer_server.lock().await;
    match server_opt.as_ref() {
        Some(server) => Ok(Some(server.status().await)),
        None => Ok(None),
    }
}

/// Process all staged WiFi transfer files through the document import pipeline.
/// Returns the number of files successfully imported.
/// Runs blocking work (file I/O + crypto + DB) in `spawn_blocking` to avoid
/// blocking the Tokio runtime.
#[tauri::command]
pub async fn process_staged_files(
    state: State<'_, Arc<CoreState>>,
) -> Result<u32, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (staging_dir, db_path) = {
            let guard = state.read_session().map_err(|e| e.to_string())?;
            let session = guard
                .as_ref()
                .ok_or("No active profile session")?;
            let staging = staging_dir_from_db_path(session.db_path())?;
            (staging, session.db_path().to_path_buf())
        };

        if !staging_dir.exists() {
            return Ok(0);
        }

        let entries: Vec<_> = std::fs::read_dir(&staging_dir)
            .map_err(|e| format!("Failed to read staging dir: {e}"))?
            .filter_map(|e| e.ok())
            .collect();

        if entries.is_empty() {
            return Ok(0);
        }

        // Re-acquire session for import_file (needs ProfileSession reference)
        let guard = state.read_session().map_err(|e| e.to_string())?;
        let session = guard
            .as_ref()
            .ok_or("No active profile session")?;

        let conn = open_database(&db_path, Some(session.key_bytes()))
            .map_err(|e| format!("Database error: {e}"))?;

        let key = session.key_bytes();
        let mut count = 0u32;
        for entry in entries {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            // SEC-02-G04: Decrypt encrypted staging file to temp for import
            let temp_file = match decrypt_staging_to_temp(&path, key) {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "WiFi transfer: staging decryption failed"
                    );
                    continue;
                }
            };

            match import_file(temp_file.path(), session, &conn) {
                Ok(result) => {
                    tracing::info!(
                        document_id = %result.document_id,
                        filename = %result.original_filename,
                        status = ?result.status,
                        "WiFi transfer file imported"
                    );
                    // Remove encrypted staged file after successful import (SEC-02-G05)
                    crate::crypto::secure_delete_file(&path).ok();
                    count += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "Failed to import WiFi transfer file"
                    );
                    // Leave failed files for retry
                }
            }
            // temp_file dropped here → auto-deleted
        }

        state.update_activity();
        Ok(count)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}
