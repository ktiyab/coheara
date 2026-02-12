use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use crate::config;
use crate::crypto::profile::ProfileSession;
use crate::wifi_transfer::TransferServer;

/// Default inactivity timeout: 15 minutes.
const DEFAULT_INACTIVITY_TIMEOUT_SECS: u64 = 900;

/// Global application state managed by Tauri.
/// Holds the active profile session, inactivity timer, and transfer server handle.
pub struct AppState {
    pub active_session: Mutex<Option<ProfileSession>>,
    pub profiles_dir: PathBuf,
    pub inactivity_timeout_secs: u64,
    pub last_activity: Mutex<Instant>,
    /// WiFi transfer server handle (L4-03). Uses tokio Mutex for async access.
    pub transfer_server: tokio::sync::Mutex<Option<TransferServer>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            active_session: Mutex::new(None),
            profiles_dir: config::profiles_dir(),
            inactivity_timeout_secs: DEFAULT_INACTIVITY_TIMEOUT_SECS,
            last_activity: Mutex::new(Instant::now()),
            transfer_server: tokio::sync::Mutex::new(None),
        }
    }

    /// Check if the profile is locked (no active session).
    pub fn is_locked(&self) -> bool {
        self.active_session
            .lock()
            .map(|guard| guard.is_none())
            .unwrap_or(true)
    }

    /// Lock the profile: drop the session (zeroes the key).
    pub fn lock(&self) {
        if let Ok(mut session) = self.active_session.lock() {
            *session = None;
        }
        tracing::info!("Profile locked");
    }

    /// Update the last activity timestamp.
    pub fn update_activity(&self) {
        if let Ok(mut last) = self.last_activity.lock() {
            *last = Instant::now();
        }
    }

    /// Check if the inactivity timeout has been exceeded.
    pub fn check_timeout(&self) -> bool {
        self.last_activity
            .lock()
            .map(|last| last.elapsed().as_secs() > self.inactivity_timeout_secs)
            .unwrap_or(false)
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_is_locked() {
        let state = AppState::new();
        assert!(state.is_locked());
    }

    #[test]
    fn lock_on_already_locked_is_safe() {
        let state = AppState::new();
        state.lock();
        assert!(state.is_locked());
    }

    #[test]
    fn update_activity_resets_timer() {
        let state = AppState::new();
        state.update_activity();
        assert!(!state.check_timeout());
    }

    #[test]
    fn check_timeout_with_zero_threshold() {
        let state = AppState {
            active_session: Mutex::new(None),
            profiles_dir: PathBuf::from("/tmp"),
            inactivity_timeout_secs: 0,
            last_activity: Mutex::new(Instant::now() - std::time::Duration::from_secs(1)),
            transfer_server: tokio::sync::Mutex::new(None),
        };
        assert!(state.check_timeout());
    }
}
