//! ME-02: Multi-Device Session Manager.
//!
//! Manages paired mobile devices, active connections, and WebSocket channels.
//! Replaces the simpler DeviceRegistry from ME-01 with full lifecycle tracking.
//!
//! Device lifecycle: PAIRED → CONNECTED → ACTIVE (WS) → DISCONNECTED → REVOKED

use std::collections::{HashMap, VecDeque};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::api::types::TokenEntry;

/// Maximum number of paired devices (configurable).
const DEFAULT_MAX_DEVICES: usize = 3;

/// Days of inactivity before showing a warning.
const INACTIVE_THRESHOLD_DAYS: i64 = 30;

/// Maximum queued alerts per device while disconnected.
const MAX_PENDING_ALERTS: usize = 50;

// ═══════════════════════════════════════════════════════════
// Error type
// ═══════════════════════════════════════════════════════════

/// Errors from device management operations.
#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("Maximum paired devices reached ({0})")]
    MaxDevicesReached(usize),
    #[error("Device not found: {0}")]
    NotFound(String),
    #[error("Device is revoked: {0}")]
    Revoked(String),
    #[error("Database error: {0}")]
    Database(String),
}

// ═══════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════

/// A paired device with full metadata.
#[derive(Debug, Clone)]
pub struct PairedDevice {
    pub device_id: String,
    pub device_name: String,
    pub device_model: String,
    pub paired_at: chrono::DateTime<chrono::Utc>,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub is_revoked: bool,
}

/// An active REST/WS connection from a device.
#[derive(Debug, Clone)]
pub struct ActiveConnection {
    pub device_id: String,
    pub connected_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub has_websocket: bool,
    pub ip_address: String,
}

/// Device summary for desktop UI display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSummary {
    pub device_id: String,
    pub device_name: String,
    pub device_model: String,
    pub paired_at: String,
    pub last_seen: String,
    pub is_connected: bool,
    pub has_websocket: bool,
    pub days_inactive: Option<i64>,
}

/// Device count for UI display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCount {
    pub paired: usize,
    pub connected: usize,
    pub max: usize,
}

/// Warning for inactive devices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InactiveWarning {
    pub device_id: String,
    pub device_name: String,
    pub last_seen: String,
    pub days_inactive: i64,
    pub message: String,
}

/// Citation reference for chat completion messages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CitationRef {
    pub document_id: String,
    pub document_title: String,
    pub chunk_id: Option<String>,
}

/// Alert detail for in-app display (behind biometric — BP-02).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertDetail {
    pub summary: String,
    pub related_document: Option<String>,
    pub severity: String,
    pub action_text: Option<String>,
}

/// Reconnection policy communicated to the phone in Welcome (IMP-020).
///
/// The phone uses these parameters for exponential backoff on disconnect:
/// `delay = min(initial_delay_ms * 2^attempt, max_delay_ms) + random_jitter`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReconnectionPolicy {
    /// Initial delay before first reconnection attempt (ms).
    pub initial_delay_ms: u32,
    /// Maximum delay cap (ms).
    pub max_delay_ms: u32,
    /// Maximum number of reconnection attempts before giving up.
    pub max_retries: u32,
    /// Maximum random jitter added to each delay (ms).
    pub jitter_ms: u32,
}

impl Default for ReconnectionPolicy {
    fn default() -> Self {
        Self {
            initial_delay_ms: 1_000,
            max_delay_ms: 30_000,
            max_retries: 10,
            jitter_ms: 500,
        }
    }
}

/// Server → Phone WebSocket messages (M0-03).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsOutgoing {
    /// Connection acknowledged. Includes reconnection backoff policy.
    Welcome {
        profile_name: String,
        session_id: String,
        reconnect_policy: ReconnectionPolicy,
    },
    /// Session expiring soon (or expired when seconds_remaining == 0).
    SessionExpiring { seconds_remaining: u32 },
    /// Device has been revoked (triggers phone-side wipe).
    Revoked {},
    /// Streaming chat token.
    ChatToken { conversation_id: String, token: String },
    /// Chat response complete with full content + citations.
    /// Phone receives the entire answer in one message (no token buffering needed).
    ChatComplete { conversation_id: String, content: String, citations: Vec<CitationRef> },
    /// Chat error during generation.
    ChatError { conversation_id: String, error: String },
    /// Critical health alert (BP-02: vague notification, detail inside app).
    CriticalAlert {
        alert_id: String,
        notification_text: String,
        detail: AlertDetail,
    },
    /// Document processing in progress.
    DocumentProcessing { document_id: String, stage: String },
    /// Document processing complete.
    DocumentComplete { document_id: String, title: String },
    /// Document processing error.
    DocumentError { document_id: String, error: String },
    /// New sync data available.
    SyncAvailable { changed_types: Vec<String> },
    /// Server heartbeat (phone should respond with Pong).
    Heartbeat { server_time: String },
    /// Active profile changed on desktop.
    ProfileChanged { profile_name: String },
}

/// Phone → Server WebSocket messages (M0-03).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsIncoming {
    /// Phone acknowledges connection.
    Ready {},
    /// Heartbeat response.
    Pong {},
    /// Chat query (streaming response via WebSocket).
    ChatQuery {
        conversation_id: Option<String>,
        message: String,
    },
    /// Feedback on a chat response.
    ChatFeedback {
        conversation_id: String,
        message_id: String,
        helpful: bool,
    },
}

// ═══════════════════════════════════════════════════════════
// DeviceManager
// ═══════════════════════════════════════════════════════════

/// Manages all paired devices, active connections, and WebSocket channels.
///
/// Lives inside `CoreState` behind a `RwLock`. The `ws_channels` field uses
/// a separate `std::sync::Mutex` because `mpsc::Sender` is not Clone-free
/// and we only need brief exclusive access.
#[derive(Debug)]
pub struct DeviceManager {
    /// All paired devices (keyed by device_id).
    devices: HashMap<String, PairedDevice>,
    /// Token entries for bearer authentication (keyed by device_id).
    tokens: HashMap<String, TokenEntry>,
    /// Currently active connections (keyed by device_id).
    active: HashMap<String, ActiveConnection>,
    /// WebSocket send channels (keyed by device_id).
    /// Not Debug-printable, so wrapped separately.
    ws_channels: HashMap<String, mpsc::Sender<WsOutgoing>>,
    /// Pending alerts queued while device is disconnected (max 50 per device).
    pending_alerts: HashMap<String, VecDeque<WsOutgoing>>,
    /// Maximum number of paired devices.
    max_devices: usize,
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceManager {
    /// Create a new empty DeviceManager.
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            tokens: HashMap::new(),
            active: HashMap::new(),
            ws_channels: HashMap::new(),
            pending_alerts: HashMap::new(),
            max_devices: DEFAULT_MAX_DEVICES,
        }
    }

    /// Load paired devices from the database, populating the devices HashMap.
    ///
    /// Call this after profile unlock to restore device state across app restarts.
    /// Tokens and active connections are NOT restored (session-specific).
    pub fn load_from_db(conn: &rusqlite::Connection) -> Result<Self, DeviceError> {
        let stored = crate::pairing::db_load_paired_devices(conn)
            .map_err(|e| DeviceError::Database(e.to_string()))?;

        let mut mgr = Self::new();
        for device in stored {
            let paired_at = chrono::DateTime::parse_from_rfc3339(&device.paired_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());
            let last_seen = chrono::DateTime::parse_from_rfc3339(&device.last_seen)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            mgr.devices.insert(
                device.device_id.clone(),
                PairedDevice {
                    device_id: device.device_id,
                    device_name: device.device_name,
                    device_model: device.device_model,
                    paired_at,
                    last_seen,
                    is_revoked: device.is_revoked,
                },
            );
        }

        Ok(mgr)
    }

    // ─── Pairing ─────────────────────────────────────────────

    /// Check if a new device can be paired (under max limit).
    pub fn can_pair(&self) -> bool {
        let active_count = self.devices.values().filter(|d| !d.is_revoked).count();
        active_count < self.max_devices
    }

    /// Register a new paired device with an initial token hash.
    /// Returns error if max devices reached.
    pub fn register_device(
        &mut self,
        device_id: String,
        device_name: String,
        device_model: String,
        token_hash: [u8; 32],
    ) -> Result<(), DeviceError> {
        if !self.can_pair() {
            return Err(DeviceError::MaxDevicesReached(self.max_devices));
        }

        let now = chrono::Utc::now();
        self.devices.insert(
            device_id.clone(),
            PairedDevice {
                device_id: device_id.clone(),
                device_name,
                device_model,
                paired_at: now,
                last_seen: now,
                is_revoked: false,
            },
        );
        self.tokens
            .insert(device_id, TokenEntry::new(token_hash));
        Ok(())
    }

    /// Update device metadata from request headers (CA-01).
    /// Called on each authenticated request to keep device info current
    /// (e.g., phone renamed, OS updated). Only updates non-empty values.
    pub fn update_device_metadata(
        &mut self,
        device_id: &str,
        name: Option<&str>,
        model: Option<&str>,
    ) {
        if let Some(device) = self.devices.get_mut(device_id) {
            if let Some(n) = name {
                if !n.is_empty() {
                    device.device_name = n.to_string();
                }
            }
            if let Some(m) = model {
                if !m.is_empty() {
                    device.device_model = m.to_string();
                }
            }
        }
    }

    /// Unpair (revoke) a device. Removes tokens and active connection.
    /// Returns the WS channel sender if one existed (caller should send Revoked).
    pub fn unpair_device(
        &mut self,
        device_id: &str,
    ) -> Result<Option<mpsc::Sender<WsOutgoing>>, DeviceError> {
        let device = self
            .devices
            .get_mut(device_id)
            .ok_or_else(|| DeviceError::NotFound(device_id.to_string()))?;

        device.is_revoked = true;
        self.tokens.remove(device_id);
        self.active.remove(device_id);
        let ws_tx = self.ws_channels.remove(device_id);
        self.pending_alerts.remove(device_id);

        Ok(ws_tx)
    }

    /// Permanently remove a revoked device from the manager.
    pub fn remove_device(&mut self, device_id: &str) -> bool {
        self.tokens.remove(device_id);
        self.active.remove(device_id);
        self.ws_channels.remove(device_id);
        self.pending_alerts.remove(device_id);
        self.devices.remove(device_id).is_some()
    }

    // ─── Token validation (used by M0-01 auth middleware) ────

    /// Validate a bearer token and rotate it.
    /// Returns `Some((device_id, device_name, new_token))` on success.
    pub fn validate_and_rotate(
        &mut self,
        token: &str,
    ) -> Option<(String, String, String)> {
        use crate::api::types::{generate_token, hash_token};

        let token_hash = hash_token(token);

        // Find which device owns this token
        let device_id = self
            .tokens
            .iter()
            .find(|(_, entry)| entry.validate(&token_hash))
            .map(|(id, _)| id.clone())?;

        // Check device is not revoked
        let device = self.devices.get(&device_id)?;
        if device.is_revoked {
            return None;
        }

        // Generate new token and rotate
        let new_token = generate_token();
        let new_hash = hash_token(&new_token);

        if let Some(entry) = self.tokens.get_mut(&device_id) {
            entry.rotate(new_hash);
        }

        // Update last_seen
        if let Some(info) = self.devices.get_mut(&device_id) {
            info.last_seen = chrono::Utc::now();
        }

        let device_name = self
            .devices
            .get(&device_id)
            .map(|d| d.device_name.clone())
            .unwrap_or_default();

        Some((device_id, device_name, new_token))
    }

    // ─── Connection tracking ─────────────────────────────────

    /// Mark a device as actively connected (on first API request).
    pub fn register_connection(&mut self, device_id: &str, ip: &str) {
        let now = chrono::Utc::now();
        self.active.insert(
            device_id.to_string(),
            ActiveConnection {
                device_id: device_id.to_string(),
                connected_at: now,
                last_activity: now,
                has_websocket: false,
                ip_address: ip.to_string(),
            },
        );
    }

    /// Remove a device from active connections.
    pub fn unregister_connection(&mut self, device_id: &str) {
        self.active.remove(device_id);
    }

    /// Update last_activity for a device (on each API request).
    pub fn touch(&mut self, device_id: &str) {
        if let Some(conn) = self.active.get_mut(device_id) {
            conn.last_activity = chrono::Utc::now();
        }
        if let Some(device) = self.devices.get_mut(device_id) {
            device.last_seen = chrono::Utc::now();
        }
    }

    /// Check if a device is currently connected.
    pub fn is_connected(&self, device_id: &str) -> bool {
        self.active.contains_key(device_id)
    }

    // ─── WebSocket channel management ────────────────────────

    /// Register a WebSocket channel for a device.
    pub fn register_ws(&mut self, device_id: &str, tx: mpsc::Sender<WsOutgoing>) {
        self.ws_channels.insert(device_id.to_string(), tx);
        if let Some(conn) = self.active.get_mut(device_id) {
            conn.has_websocket = true;
        }
    }

    /// Remove a WebSocket channel (on disconnect).
    pub fn unregister_ws(&mut self, device_id: &str) {
        self.ws_channels.remove(device_id);
        if let Some(conn) = self.active.get_mut(device_id) {
            conn.has_websocket = false;
        }
    }

    /// Get a clone of the WS sender for a device (for async sending outside lock).
    pub fn ws_sender(&self, device_id: &str) -> Option<mpsc::Sender<WsOutgoing>> {
        self.ws_channels.get(device_id).cloned()
    }

    /// Get all WS senders (for broadcast outside lock).
    pub fn all_ws_senders(&self) -> Vec<(String, mpsc::Sender<WsOutgoing>)> {
        self.ws_channels
            .iter()
            .map(|(id, tx)| (id.clone(), tx.clone()))
            .collect()
    }

    // ─── Alert queue (when device disconnected) ──────────────

    /// Send a message to a device, or queue it if disconnected/full.
    pub fn send_or_queue(&mut self, device_id: &str, msg: WsOutgoing) {
        if let Some(tx) = self.ws_channels.get(device_id) {
            match tx.try_send(msg) {
                Ok(()) => (),
                Err(mpsc::error::TrySendError::Full(msg))
                | Err(mpsc::error::TrySendError::Closed(msg)) => {
                    self.queue_alert(device_id, msg);
                }
            }
        } else {
            self.queue_alert(device_id, msg);
        }
    }

    fn queue_alert(&mut self, device_id: &str, msg: WsOutgoing) {
        let queue = self
            .pending_alerts
            .entry(device_id.to_string())
            .or_default();
        if queue.len() < MAX_PENDING_ALERTS {
            queue.push_back(msg);
        }
        // Silently drop if at capacity (50)
    }

    /// Flush pending alerts through the device's WS channel.
    pub fn flush_pending(&mut self, device_id: &str) {
        let tx = match self.ws_channels.get(device_id) {
            Some(tx) => tx.clone(),
            None => return,
        };
        if let Some(queue) = self.pending_alerts.get_mut(device_id) {
            while let Some(msg) = queue.pop_front() {
                if tx.try_send(msg).is_err() {
                    break;
                }
            }
        }
    }

    /// Number of pending alerts for a device.
    pub fn pending_count(&self, device_id: &str) -> usize {
        self.pending_alerts
            .get(device_id)
            .map(|q| q.len())
            .unwrap_or(0)
    }

    /// Broadcast a message to all paired (non-revoked) devices.
    pub fn broadcast(&mut self, msg: WsOutgoing) {
        let device_ids: Vec<String> = self
            .devices
            .values()
            .filter(|d| !d.is_revoked)
            .map(|d| d.device_id.clone())
            .collect();
        for device_id in device_ids {
            self.send_or_queue(&device_id, msg.clone());
        }
    }

    // ─── Queries ─────────────────────────────────────────────

    /// Number of paired (non-revoked) devices.
    pub fn device_count(&self) -> usize {
        self.devices.values().filter(|d| !d.is_revoked).count()
    }

    /// Number of currently connected devices.
    pub fn connected_count(&self) -> usize {
        self.active.len()
    }

    /// Check if a device is paired (including revoked).
    pub fn is_paired(&self, device_id: &str) -> bool {
        self.devices.contains_key(device_id)
    }

    /// Get device info by ID.
    pub fn get_device(&self, device_id: &str) -> Option<&PairedDevice> {
        self.devices.get(device_id)
    }

    /// Get device count summary.
    pub fn count(&self) -> DeviceCount {
        DeviceCount {
            paired: self.device_count(),
            connected: self.connected_count(),
            max: self.max_devices,
        }
    }

    /// List all paired devices with connection status.
    pub fn list_devices(&self) -> Vec<DeviceSummary> {
        let now = chrono::Utc::now();
        self.devices
            .values()
            .filter(|d| !d.is_revoked)
            .map(|d| {
                let is_connected = self.active.contains_key(&d.device_id);
                let has_websocket = self
                    .active
                    .get(&d.device_id)
                    .map(|c| c.has_websocket)
                    .unwrap_or(false);
                let days_since = (now - d.last_seen).num_days();
                DeviceSummary {
                    device_id: d.device_id.clone(),
                    device_name: d.device_name.clone(),
                    device_model: d.device_model.clone(),
                    paired_at: d.paired_at.to_rfc3339(),
                    last_seen: d.last_seen.to_rfc3339(),
                    is_connected,
                    has_websocket,
                    days_inactive: if days_since > 0 {
                        Some(days_since)
                    } else {
                        None
                    },
                }
            })
            .collect()
    }

    /// Get devices inactive for more than the threshold.
    pub fn inactive_devices(&self) -> Vec<InactiveWarning> {
        let now = chrono::Utc::now();
        self.devices
            .values()
            .filter(|d| !d.is_revoked)
            .filter_map(|d| {
                let days = (now - d.last_seen).num_days();
                if days >= INACTIVE_THRESHOLD_DAYS {
                    Some(InactiveWarning {
                        device_id: d.device_id.clone(),
                        device_name: d.device_name.clone(),
                        last_seen: d.last_seen.to_rfc3339(),
                        days_inactive: days,
                        message: format!(
                            "{} hasn't connected in {} days. Consider unpairing for security.",
                            d.device_name, days
                        ),
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::{generate_token, hash_token};

    fn make_manager_with_device() -> (DeviceManager, String) {
        let mut mgr = DeviceManager::new();
        let token = generate_token();
        let hash = hash_token(&token);
        mgr.register_device(
            "dev-1".into(),
            "Test Phone".into(),
            "iPhone 15".into(),
            hash,
        )
        .unwrap();
        (mgr, token)
    }

    // ── Pairing tests ────────────────────────────────────────

    #[test]
    fn new_manager_is_empty() {
        let mgr = DeviceManager::new();
        assert_eq!(mgr.device_count(), 0);
        assert!(mgr.can_pair());
        assert!(!mgr.is_paired("any"));
    }

    #[test]
    fn register_device_success() {
        let (mgr, _) = make_manager_with_device();
        assert_eq!(mgr.device_count(), 1);
        assert!(mgr.is_paired("dev-1"));
        let device = mgr.get_device("dev-1").unwrap();
        assert_eq!(device.device_name, "Test Phone");
        assert_eq!(device.device_model, "iPhone 15");
        assert!(!device.is_revoked);
    }

    #[test]
    fn update_device_metadata_updates_name_and_model() {
        let (mut mgr, _) = make_manager_with_device();
        mgr.update_device_metadata("dev-1", Some("New Name"), Some("Pixel 9"));
        let device = mgr.get_device("dev-1").unwrap();
        assert_eq!(device.device_name, "New Name");
        assert_eq!(device.device_model, "Pixel 9");
    }

    #[test]
    fn update_device_metadata_skips_empty_values() {
        let (mut mgr, _) = make_manager_with_device();
        mgr.update_device_metadata("dev-1", Some(""), None);
        let device = mgr.get_device("dev-1").unwrap();
        assert_eq!(device.device_name, "Test Phone"); // Unchanged
        assert_eq!(device.device_model, "iPhone 15"); // Unchanged
    }

    #[test]
    fn update_device_metadata_ignores_unknown_device() {
        let (mut mgr, _) = make_manager_with_device();
        mgr.update_device_metadata("unknown", Some("X"), Some("Y")); // No panic
        assert!(mgr.get_device("unknown").is_none());
    }

    #[test]
    fn register_device_respects_max_limit() {
        let mut mgr = DeviceManager::new();
        for i in 0..3 {
            let token = generate_token();
            let hash = hash_token(&token);
            mgr.register_device(
                format!("dev-{i}"),
                format!("Phone {i}"),
                "Model".into(),
                hash,
            )
            .unwrap();
        }
        assert!(!mgr.can_pair());

        let token = generate_token();
        let hash = hash_token(&token);
        let result = mgr.register_device("dev-3".into(), "Phone 3".into(), "Model".into(), hash);
        assert!(matches!(result, Err(DeviceError::MaxDevicesReached(3))));
    }

    #[test]
    fn unpair_frees_slot() {
        let mut mgr = DeviceManager::new();
        for i in 0..3 {
            let token = generate_token();
            let hash = hash_token(&token);
            mgr.register_device(
                format!("dev-{i}"),
                format!("Phone {i}"),
                "Model".into(),
                hash,
            )
            .unwrap();
        }
        assert!(!mgr.can_pair());

        mgr.unpair_device("dev-1").unwrap();
        assert!(mgr.can_pair());
    }

    #[test]
    fn unpair_nonexistent_device_errors() {
        let mut mgr = DeviceManager::new();
        let result = mgr.unpair_device("nonexistent");
        assert!(matches!(result, Err(DeviceError::NotFound(_))));
    }

    #[test]
    fn remove_device_cleans_up() {
        let (mut mgr, _) = make_manager_with_device();
        assert!(mgr.remove_device("dev-1"));
        assert_eq!(mgr.device_count(), 0);
        assert!(!mgr.is_paired("dev-1"));
    }

    // ── Token validation tests ───────────────────────────────

    #[test]
    fn validate_and_rotate_success() {
        let (mut mgr, token) = make_manager_with_device();
        let result = mgr.validate_and_rotate(&token);
        assert!(result.is_some());
        let (device_id, device_name, new_token) = result.unwrap();
        assert_eq!(device_id, "dev-1");
        assert_eq!(device_name, "Test Phone");
        assert!(!new_token.is_empty());
        assert_ne!(new_token, token);
    }

    #[test]
    fn validate_rejects_invalid_token() {
        let (mut mgr, _) = make_manager_with_device();
        let result = mgr.validate_and_rotate("invalid-token");
        assert!(result.is_none());
    }

    #[test]
    fn validate_rejects_revoked_device() {
        let (mut mgr, token) = make_manager_with_device();
        mgr.unpair_device("dev-1").unwrap();
        let result = mgr.validate_and_rotate(&token);
        assert!(result.is_none());
    }

    #[test]
    fn rotated_token_works() {
        let (mut mgr, token) = make_manager_with_device();
        let (_, _, new_token) = mgr.validate_and_rotate(&token).unwrap();
        let result = mgr.validate_and_rotate(&new_token);
        assert!(result.is_some());
    }

    // ── Connection tracking tests ────────────────────────────

    #[test]
    fn register_connection_tracks_device() {
        let (mut mgr, _) = make_manager_with_device();
        assert!(!mgr.is_connected("dev-1"));

        mgr.register_connection("dev-1", "192.168.1.100");
        assert!(mgr.is_connected("dev-1"));
        assert_eq!(mgr.connected_count(), 1);
    }

    #[test]
    fn unregister_connection_removes_tracking() {
        let (mut mgr, _) = make_manager_with_device();
        mgr.register_connection("dev-1", "192.168.1.100");
        assert!(mgr.is_connected("dev-1"));

        mgr.unregister_connection("dev-1");
        assert!(!mgr.is_connected("dev-1"));
        assert_eq!(mgr.connected_count(), 0);
    }

    #[test]
    fn touch_updates_activity() {
        let (mut mgr, _) = make_manager_with_device();
        let before = mgr.get_device("dev-1").unwrap().last_seen;
        std::thread::sleep(std::time::Duration::from_millis(10));
        mgr.touch("dev-1");
        let after = mgr.get_device("dev-1").unwrap().last_seen;
        assert!(after >= before);
    }

    // ── WebSocket channel tests ──────────────────────────────

    #[test]
    fn register_ws_marks_connection() {
        let (mut mgr, _) = make_manager_with_device();
        mgr.register_connection("dev-1", "192.168.1.100");

        let (tx, _rx) = mpsc::channel::<WsOutgoing>(16);
        mgr.register_ws("dev-1", tx);

        let summary = mgr.list_devices();
        let dev = summary.iter().find(|d| d.device_id == "dev-1").unwrap();
        assert!(dev.has_websocket);
    }

    #[test]
    fn unregister_ws_clears_channel() {
        let (mut mgr, _) = make_manager_with_device();
        mgr.register_connection("dev-1", "192.168.1.100");

        let (tx, _rx) = mpsc::channel::<WsOutgoing>(16);
        mgr.register_ws("dev-1", tx);
        assert!(mgr.ws_sender("dev-1").is_some());

        mgr.unregister_ws("dev-1");
        assert!(mgr.ws_sender("dev-1").is_none());
    }

    #[test]
    fn all_ws_senders_returns_connected() {
        let mut mgr = DeviceManager::new();
        for i in 0..2 {
            let token = generate_token();
            let hash = hash_token(&token);
            mgr.register_device(
                format!("dev-{i}"),
                format!("Phone {i}"),
                "Model".into(),
                hash,
            )
            .unwrap();
            mgr.register_connection(&format!("dev-{i}"), "192.168.1.100");
            let (tx, _rx) = mpsc::channel::<WsOutgoing>(16);
            mgr.register_ws(&format!("dev-{i}"), tx);
        }
        assert_eq!(mgr.all_ws_senders().len(), 2);
    }

    // ── Query tests ──────────────────────────────────────────

    #[test]
    fn list_devices_includes_status() {
        let (mut mgr, _) = make_manager_with_device();
        mgr.register_connection("dev-1", "192.168.1.100");

        let list = mgr.list_devices();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].device_id, "dev-1");
        assert!(list[0].is_connected);
    }

    #[test]
    fn list_devices_excludes_revoked() {
        let (mut mgr, _) = make_manager_with_device();
        mgr.unpair_device("dev-1").unwrap();

        let list = mgr.list_devices();
        assert!(list.is_empty());
    }

    #[test]
    fn device_count_summary() {
        let (mut mgr, _) = make_manager_with_device();
        mgr.register_connection("dev-1", "192.168.1.100");

        let count = mgr.count();
        assert_eq!(count.paired, 1);
        assert_eq!(count.connected, 1);
        assert_eq!(count.max, 3);
    }

    #[test]
    fn inactive_devices_detection() {
        let mut mgr = DeviceManager::new();
        let token = generate_token();
        let hash = hash_token(&token);
        mgr.register_device("dev-old".into(), "Old Phone".into(), "Model".into(), hash)
            .unwrap();

        // Manually set last_seen to 35 days ago
        if let Some(device) = mgr.devices.get_mut("dev-old") {
            device.last_seen = chrono::Utc::now() - chrono::Duration::days(35);
        }

        let inactive = mgr.inactive_devices();
        assert_eq!(inactive.len(), 1);
        assert_eq!(inactive[0].device_id, "dev-old");
        assert_eq!(inactive[0].days_inactive, 35);
        assert!(inactive[0].message.contains("35 days"));
    }

    #[test]
    fn active_device_not_in_inactive_list() {
        let (mgr, _) = make_manager_with_device();
        let inactive = mgr.inactive_devices();
        assert!(inactive.is_empty());
    }

    // ── Unpair with WS channel test ──────────────────────────

    #[test]
    fn unpair_returns_ws_sender() {
        let (mut mgr, _) = make_manager_with_device();
        mgr.register_connection("dev-1", "192.168.1.100");
        let (tx, _rx) = mpsc::channel::<WsOutgoing>(16);
        mgr.register_ws("dev-1", tx);

        let result = mgr.unpair_device("dev-1").unwrap();
        assert!(result.is_some()); // WS sender returned for caller to send Revoked
    }

    #[test]
    fn unpair_without_ws_returns_none() {
        let (mut mgr, _) = make_manager_with_device();
        let result = mgr.unpair_device("dev-1").unwrap();
        assert!(result.is_none());
    }

    // ── Multiple devices test ────────────────────────────────

    #[test]
    fn multiple_devices_independent() {
        let mut mgr = DeviceManager::new();
        let tokens: Vec<_> = (0..3)
            .map(|i| {
                let token = generate_token();
                let hash = hash_token(&token);
                mgr.register_device(
                    format!("dev-{i}"),
                    format!("Phone {i}"),
                    "Model".into(),
                    hash,
                )
                .unwrap();
                token
            })
            .collect();

        // Connect two devices
        mgr.register_connection("dev-0", "192.168.1.10");
        mgr.register_connection("dev-1", "192.168.1.11");

        // Validate tokens independently
        let r0 = mgr.validate_and_rotate(&tokens[0]);
        assert!(r0.is_some());
        assert_eq!(r0.unwrap().0, "dev-0");

        let r2 = mgr.validate_and_rotate(&tokens[2]);
        assert!(r2.is_some());
        assert_eq!(r2.unwrap().0, "dev-2");

        // Unpair one doesn't affect others
        mgr.unpair_device("dev-1").unwrap();
        assert_eq!(mgr.device_count(), 2);
        assert!(mgr.is_connected("dev-0"));
    }

    // ── WsOutgoing serialization tests (M0-03) ───────────────

    #[test]
    fn ws_outgoing_welcome_serializes() {
        let msg = WsOutgoing::Welcome {
            profile_name: "John".into(),
            session_id: "abc-123".into(),
            reconnect_policy: ReconnectionPolicy::default(),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "Welcome");
        assert_eq!(json["profile_name"], "John");
        assert_eq!(json["session_id"], "abc-123");
        assert!(json["reconnect_policy"].is_object());
    }

    #[test]
    fn ws_outgoing_heartbeat_serializes() {
        let msg = WsOutgoing::Heartbeat {
            server_time: "2025-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "Heartbeat");
        assert_eq!(json["server_time"], "2025-01-01T00:00:00Z");
    }

    #[test]
    fn ws_outgoing_critical_alert_serializes() {
        let msg = WsOutgoing::CriticalAlert {
            alert_id: "a-1".into(),
            notification_text: "New health observation".into(),
            detail: AlertDetail {
                summary: "Potassium result".into(),
                related_document: Some("Lab results".into()),
                severity: "attention".into(),
                action_text: Some("Discuss with doctor".into()),
            },
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "CriticalAlert");
        assert_eq!(json["notification_text"], "New health observation");
        assert_eq!(json["detail"]["summary"], "Potassium result");
    }

    #[test]
    fn ws_outgoing_revoked_serializes() {
        let msg = WsOutgoing::Revoked {};
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "Revoked");
    }

    #[test]
    fn ws_outgoing_chat_complete_serializes() {
        let msg = WsOutgoing::ChatComplete {
            conversation_id: "conv-1".into(),
            content: "Based on your records, your HbA1c is 7.2%.".into(),
            citations: vec![CitationRef {
                document_id: "doc-1".into(),
                document_title: "Lab Report".into(),
                chunk_id: Some("c-1".into()),
            }],
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "ChatComplete");
        assert_eq!(json["content"], "Based on your records, your HbA1c is 7.2%.");
        assert_eq!(json["citations"][0]["document_title"], "Lab Report");
    }

    #[test]
    fn ws_outgoing_sync_available_serializes() {
        let msg = WsOutgoing::SyncAvailable {
            changed_types: vec!["medications".into(), "labs".into()],
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "SyncAvailable");
        assert_eq!(json["changed_types"][0], "medications");
    }

    #[test]
    fn ws_outgoing_roundtrip() {
        let msg = WsOutgoing::ChatToken {
            conversation_id: "c-1".into(),
            token: "Based".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let roundtrip: WsOutgoing = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, roundtrip);
    }

    // ── WsIncoming deserialization tests ──────────────────────

    #[test]
    fn ws_incoming_ready_deserializes() {
        let json = r#"{"type":"Ready"}"#;
        let msg: WsIncoming = serde_json::from_str(json).unwrap();
        assert_eq!(msg, WsIncoming::Ready {});
    }

    #[test]
    fn ws_incoming_pong_deserializes() {
        let json = r#"{"type":"Pong"}"#;
        let msg: WsIncoming = serde_json::from_str(json).unwrap();
        assert_eq!(msg, WsIncoming::Pong {});
    }

    #[test]
    fn ws_incoming_chat_query_deserializes() {
        let json = r#"{"type":"ChatQuery","conversation_id":null,"message":"What meds?"}"#;
        let msg: WsIncoming = serde_json::from_str(json).unwrap();
        assert_eq!(
            msg,
            WsIncoming::ChatQuery {
                conversation_id: None,
                message: "What meds?".into(),
            }
        );
    }

    #[test]
    fn ws_incoming_chat_feedback_deserializes() {
        let json = r#"{"type":"ChatFeedback","conversation_id":"c-1","message_id":"m-1","helpful":true}"#;
        let msg: WsIncoming = serde_json::from_str(json).unwrap();
        assert_eq!(
            msg,
            WsIncoming::ChatFeedback {
                conversation_id: "c-1".into(),
                message_id: "m-1".into(),
                helpful: true,
            }
        );
    }

    #[test]
    fn ws_incoming_roundtrip() {
        let msg = WsIncoming::ChatQuery {
            conversation_id: Some("c-1".into()),
            message: "Tell me about my medications".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let roundtrip: WsIncoming = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, roundtrip);
    }

    // ── Alert queue tests (M0-03) ─────────────────────────────

    #[test]
    fn send_or_queue_delivers_when_connected() {
        let (mut mgr, _) = make_manager_with_device();
        mgr.register_connection("dev-1", "192.168.1.100");
        let (tx, mut rx) = mpsc::channel::<WsOutgoing>(16);
        mgr.register_ws("dev-1", tx);

        let msg = WsOutgoing::Heartbeat {
            server_time: "now".into(),
        };
        mgr.send_or_queue("dev-1", msg.clone());

        let received = rx.try_recv().unwrap();
        assert_eq!(received, msg);
    }

    #[test]
    fn send_or_queue_queues_when_disconnected() {
        let (mut mgr, _) = make_manager_with_device();
        let msg = WsOutgoing::Heartbeat {
            server_time: "now".into(),
        };
        mgr.send_or_queue("dev-1", msg);
        assert_eq!(mgr.pending_count("dev-1"), 1);
    }

    #[test]
    fn flush_pending_delivers_queued() {
        let (mut mgr, _) = make_manager_with_device();
        for i in 0..3 {
            mgr.send_or_queue(
                "dev-1",
                WsOutgoing::Heartbeat {
                    server_time: format!("t{i}"),
                },
            );
        }
        assert_eq!(mgr.pending_count("dev-1"), 3);

        let (tx, mut rx) = mpsc::channel::<WsOutgoing>(16);
        mgr.register_ws("dev-1", tx);
        mgr.flush_pending("dev-1");

        assert_eq!(mgr.pending_count("dev-1"), 0);
        for i in 0..3 {
            let msg = rx.try_recv().unwrap();
            assert_eq!(
                msg,
                WsOutgoing::Heartbeat {
                    server_time: format!("t{i}"),
                }
            );
        }
    }

    #[test]
    fn queue_overflow_drops_excess() {
        let (mut mgr, _) = make_manager_with_device();
        for i in 0..55 {
            mgr.send_or_queue(
                "dev-1",
                WsOutgoing::Heartbeat {
                    server_time: format!("t{i}"),
                },
            );
        }
        assert_eq!(mgr.pending_count("dev-1"), 50);
    }

    #[test]
    fn broadcast_reaches_all_devices() {
        let mut mgr = DeviceManager::new();
        let mut receivers = Vec::new();
        for i in 0..2 {
            let token = generate_token();
            let hash = hash_token(&token);
            mgr.register_device(
                format!("dev-{i}"),
                format!("Phone {i}"),
                "Model".into(),
                hash,
            )
            .unwrap();
            mgr.register_connection(&format!("dev-{i}"), "192.168.1.100");
            let (tx, rx) = mpsc::channel::<WsOutgoing>(16);
            mgr.register_ws(&format!("dev-{i}"), tx);
            receivers.push(rx);
        }

        let msg = WsOutgoing::ProfileChanged {
            profile_name: "Alice".into(),
        };
        mgr.broadcast(msg.clone());

        for mut rx in receivers {
            let received = rx.try_recv().unwrap();
            assert_eq!(received, msg);
        }
    }

    #[test]
    fn broadcast_queues_for_disconnected() {
        let mut mgr = DeviceManager::new();
        for i in 0..2 {
            let token = generate_token();
            let hash = hash_token(&token);
            mgr.register_device(
                format!("dev-{i}"),
                format!("Phone {i}"),
                "Model".into(),
                hash,
            )
            .unwrap();
        }
        // Only connect dev-0
        let (tx, mut rx0) = mpsc::channel::<WsOutgoing>(16);
        mgr.register_ws("dev-0", tx);

        let msg = WsOutgoing::SyncAvailable {
            changed_types: vec!["medications".into()],
        };
        mgr.broadcast(msg.clone());

        assert_eq!(rx0.try_recv().unwrap(), msg);
        assert_eq!(mgr.pending_count("dev-1"), 1);
    }

    #[test]
    fn unpair_clears_pending_alerts() {
        let (mut mgr, _) = make_manager_with_device();
        mgr.send_or_queue(
            "dev-1",
            WsOutgoing::Heartbeat {
                server_time: "now".into(),
            },
        );
        assert_eq!(mgr.pending_count("dev-1"), 1);

        mgr.unpair_device("dev-1").unwrap();
        assert_eq!(mgr.pending_count("dev-1"), 0);
    }

    #[test]
    fn remove_device_clears_pending_alerts() {
        let (mut mgr, _) = make_manager_with_device();
        mgr.send_or_queue(
            "dev-1",
            WsOutgoing::Heartbeat {
                server_time: "now".into(),
            },
        );
        assert!(mgr.remove_device("dev-1"));
        assert_eq!(mgr.pending_count("dev-1"), 0);
    }

    // ── Database persistence tests (RS-ME-02-001) ─────────

    #[test]
    fn load_from_db_restores_paired_devices() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let pk: [u8; 32] = rand::random();

        crate::pairing::db_store_paired_device(&conn, "dev-1", "Test Phone", "iPhone 15", &pk)
            .unwrap();

        let mgr = DeviceManager::load_from_db(&conn).unwrap();
        assert_eq!(mgr.device_count(), 1);
        assert!(mgr.is_paired("dev-1"));

        let device = mgr.get_device("dev-1").unwrap();
        assert_eq!(device.device_name, "Test Phone");
        assert_eq!(device.device_model, "iPhone 15");
        assert!(!device.is_revoked);
    }

    #[test]
    fn load_from_db_restores_revoked_devices() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let pk: [u8; 32] = rand::random();

        crate::pairing::db_store_paired_device(&conn, "dev-1", "Phone", "Model", &pk).unwrap();
        crate::pairing::db_revoke_device(&conn, "dev-1").unwrap();

        let mgr = DeviceManager::load_from_db(&conn).unwrap();
        assert!(mgr.is_paired("dev-1"));

        let device = mgr.get_device("dev-1").unwrap();
        assert!(device.is_revoked);
        // Revoked devices don't count toward active count
        assert_eq!(mgr.device_count(), 0);
    }

    #[test]
    fn load_from_db_empty_database() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let mgr = DeviceManager::load_from_db(&conn).unwrap();
        assert_eq!(mgr.device_count(), 0);
    }

    #[test]
    fn load_from_db_multiple_devices() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();

        for i in 0..3 {
            let pk: [u8; 32] = rand::random();
            crate::pairing::db_store_paired_device(
                &conn,
                &format!("dev-{i}"),
                &format!("Phone {i}"),
                "Model",
                &pk,
            )
            .unwrap();
        }

        let mgr = DeviceManager::load_from_db(&conn).unwrap();
        assert_eq!(mgr.device_count(), 3);
        for i in 0..3 {
            assert!(mgr.is_paired(&format!("dev-{i}")));
        }
    }

    #[test]
    fn load_from_db_preserves_timestamps() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let pk: [u8; 32] = rand::random();

        crate::pairing::db_store_paired_device(&conn, "dev-1", "Phone", "Model", &pk).unwrap();

        let mgr = DeviceManager::load_from_db(&conn).unwrap();
        let device = mgr.get_device("dev-1").unwrap();

        // Timestamps should be valid (not default epoch)
        let epoch = chrono::DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert!(device.paired_at > epoch);
        assert!(device.last_seen > epoch);
    }

    // ── ReconnectionPolicy tests (IMP-020) ────────────────────

    #[test]
    fn reconnection_policy_default_values() {
        let policy = ReconnectionPolicy::default();
        assert_eq!(policy.initial_delay_ms, 1_000);
        assert_eq!(policy.max_delay_ms, 30_000);
        assert_eq!(policy.max_retries, 10);
        assert_eq!(policy.jitter_ms, 500);
    }

    #[test]
    fn reconnection_policy_serializes_in_welcome() {
        let welcome = WsOutgoing::Welcome {
            profile_name: "Test".into(),
            session_id: "sess-123".into(),
            reconnect_policy: ReconnectionPolicy::default(),
        };
        let json = serde_json::to_value(&welcome).unwrap();
        assert_eq!(json["type"], "Welcome");
        assert_eq!(json["reconnect_policy"]["initial_delay_ms"], 1000);
        assert_eq!(json["reconnect_policy"]["max_delay_ms"], 30000);
        assert_eq!(json["reconnect_policy"]["max_retries"], 10);
        assert_eq!(json["reconnect_policy"]["jitter_ms"], 500);
    }

    #[test]
    fn reconnection_policy_max_delay_exceeds_initial() {
        let policy = ReconnectionPolicy::default();
        assert!(
            policy.max_delay_ms > policy.initial_delay_ms,
            "max_delay must exceed initial_delay for exponential backoff"
        );
    }
}
