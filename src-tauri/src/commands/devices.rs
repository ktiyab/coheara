//! ME-02: Device management IPC commands.
//!
//! Desktop Tauri commands for managing paired mobile devices.

use std::sync::Arc;

use tauri::State;

use crate::core_state::CoreState;
use crate::device_manager::{DeviceCount, DeviceSummary, InactiveWarning};

/// List all paired devices with connection status.
#[tauri::command]
pub fn list_paired_devices(
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<DeviceSummary>, String> {
    let devices = state.read_devices().map_err(|e| e.to_string())?;
    Ok(devices.list_devices())
}

/// Unpair (revoke) a device by ID.
#[tauri::command]
pub fn unpair_device(
    state: State<'_, Arc<CoreState>>,
    device_id: String,
) -> Result<(), String> {
    state.log_access(
        crate::core_state::AccessSource::DesktopUi,
        "unpair_device",
        &device_id,
    );

    let ws_tx = {
        let mut devices = state.write_devices().map_err(|e| e.to_string())?;
        devices
            .unpair_device(&device_id)
            .map_err(|e| e.to_string())?
    };

    // Send revocation to connected device if it has a WebSocket
    if let Some(tx) = ws_tx {
        let _ = tx.try_send(crate::device_manager::WsOutgoing::Revoked {});
    }

    Ok(())
}

/// Get device count summary (paired, connected, max).
#[tauri::command]
pub fn get_device_count(
    state: State<'_, Arc<CoreState>>,
) -> Result<DeviceCount, String> {
    let devices = state.read_devices().map_err(|e| e.to_string())?;
    Ok(devices.count())
}

/// Get inactive device warnings (30+ days without connection).
#[tauri::command]
pub fn get_inactive_warnings(
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<InactiveWarning>, String> {
    let devices = state.read_devices().map_err(|e| e.to_string())?;
    Ok(devices.inactive_devices())
}
