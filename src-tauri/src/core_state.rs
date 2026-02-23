//! ME-01: Transport-agnostic application state.
//!
//! `CoreState` replaces `AppState` as the single shared state between
//! Tauri IPC (desktop) and axum REST (mobile). Uses `RwLock` for
//! concurrent read access from multiple transports.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::Instant;

use uuid::Uuid;

use crate::api::MobileApiServer;
use crate::config;
use crate::crypto::profile::ProfileSession;
use crate::db;
use crate::device_manager::DeviceManager;
use crate::distribution::DistributionServer;
use crate::ollama_service::OllamaService;
use crate::pairing::PairingManager;
use crate::pipeline::structuring::preferences::ActiveModelResolver;
use crate::session_cache::{CachedSession, SessionCache, SessionCacheError};
use crate::wifi_transfer::TransferServer;

/// Default inactivity timeout: 15 minutes.
const DEFAULT_INACTIVITY_TIMEOUT_SECS: u64 = 900;

/// Maximum audit buffer size before flush.
const AUDIT_BUFFER_CAPACITY: usize = 100;

// ═══════════════════════════════════════════════════════════
// CoreState — shared by Tauri IPC and axum REST
// ═══════════════════════════════════════════════════════════

/// Transport-agnostic application state.
///
/// Wrapped in `Arc` at startup so both Tauri and axum share
/// the same instance. Uses `RwLock` for the session to allow
/// concurrent reads (most operations) while blocking only on
/// writes (login/logout).
pub struct CoreState {
    /// Active profile session (unlocked profile). `None` when locked.
    session: RwLock<Option<ProfileSession>>,
    /// MP-01: Multi-profile session cache (iOS Keychain pattern).
    /// Holds unlocked profile keys for companion device access.
    session_cache: RwLock<SessionCache>,
    /// Directory containing all profile folders.
    pub profiles_dir: PathBuf,
    /// Inactivity timeout threshold in seconds.
    pub inactivity_timeout_secs: u64,
    /// Last user interaction timestamp.
    last_activity: Mutex<Instant>,
    /// WiFi transfer server handle (L4-03). Uses tokio Mutex for async.
    pub transfer_server: tokio::sync::Mutex<Option<TransferServer>>,
    /// App distribution server handle (ADS). Uses tokio Mutex for async.
    pub distribution_server: tokio::sync::Mutex<Option<DistributionServer>>,
    /// Mobile API server handle (E2E-B06). Uses tokio Mutex for async.
    pub api_server: tokio::sync::Mutex<Option<MobileApiServer>>,
    /// Paired mobile devices — ME-02 DeviceManager.
    devices: RwLock<DeviceManager>,
    /// Device pairing protocol — M0-02 PairingManager.
    pairing: Mutex<PairingManager>,
    /// Audit log for all data access events.
    audit: AuditLogger,
    /// L6-04: Model preference resolver (singleton, shared cache).
    model_resolver: ActiveModelResolver,
    /// Centralized Ollama access — exclusive lock + operation tracking.
    ollama_service: OllamaService,
    /// S.1: Whether AI generation has been verified since last check.
    /// Set to true by `verify_ai_status`, cleared on degraded/error events.
    ai_verified: AtomicBool,
}

impl CoreState {
    /// Create a new CoreState with defaults.
    pub fn new() -> Self {
        Self {
            session: RwLock::new(None),
            session_cache: RwLock::new(SessionCache::new()),
            profiles_dir: config::profiles_dir(),
            inactivity_timeout_secs: DEFAULT_INACTIVITY_TIMEOUT_SECS,
            last_activity: Mutex::new(Instant::now()),
            transfer_server: tokio::sync::Mutex::new(None),
            distribution_server: tokio::sync::Mutex::new(None),
            api_server: tokio::sync::Mutex::new(None),
            devices: RwLock::new(DeviceManager::new()),
            pairing: Mutex::new(PairingManager::new()),
            audit: AuditLogger::new(),
            model_resolver: ActiveModelResolver::new(),
            ollama_service: OllamaService::new(),
            ai_verified: AtomicBool::new(false),
        }
    }

    // ── Session access (read path) ──────────────────────────

    /// Acquire a read lock on the session.
    ///
    /// Most command handlers use this to borrow `ProfileSession`
    /// without cloning (ProfileKey uses zeroize, cannot Clone).
    pub fn read_session(
        &self,
    ) -> Result<RwLockReadGuard<'_, Option<ProfileSession>>, CoreError> {
        self.session.read().map_err(|_| CoreError::LockPoisoned)
    }

    /// Open a database connection for the active session.
    ///
    /// Acquires a read lock, reads the db path, opens connection,
    /// then releases the lock. Most common operation in handlers.
    pub fn open_db(&self) -> Result<rusqlite::Connection, CoreError> {
        let guard = self.session.read().map_err(|_| CoreError::LockPoisoned)?;
        let session = guard.as_ref().ok_or(CoreError::NoActiveSession)?;
        db::open_database(session.db_path(), Some(session.key_bytes()))
            .map_err(CoreError::Database)
    }

    /// Get the database path for the active session (owned copy).
    ///
    /// Needed by components that open their own connections (e.g. SqliteVectorStore).
    pub fn db_path(&self) -> Result<std::path::PathBuf, CoreError> {
        let guard = self.session.read().map_err(|_| CoreError::LockPoisoned)?;
        let session = guard.as_ref().ok_or(CoreError::NoActiveSession)?;
        Ok(session.db_path().to_path_buf())
    }

    /// Get the database encryption key for the active session (owned copy).
    ///
    /// Needed by components that open their own connections and require
    /// SQLCipher encryption (e.g. SqliteVectorStore, storage orchestrator).
    pub fn db_key(&self) -> Result<[u8; 32], CoreError> {
        let guard = self.session.read().map_err(|_| CoreError::LockPoisoned)?;
        let session = guard.as_ref().ok_or(CoreError::NoActiveSession)?;
        Ok(*session.key_bytes())
    }

    // ── Session mutation (write path) ───────────────────────

    /// Acquire a write lock on the session.
    pub fn write_session(
        &self,
    ) -> Result<RwLockWriteGuard<'_, Option<ProfileSession>>, CoreError> {
        self.session.write().map_err(|_| CoreError::LockPoisoned)
    }

    /// Set active session (login/unlock).
    ///
    /// Also dual-writes to SessionCache so companion devices can access
    /// the active profile without additional unlock.
    pub fn set_session(&self, session: ProfileSession) -> Result<(), CoreError> {
        // Dual-write to SessionCache: cache key bytes before moving session
        let cached = CachedSession::new(
            session.profile_id,
            session.profile_name.clone(),
            *session.key_bytes(),
            session.db_path().to_path_buf(),
        );
        if let Ok(mut cache) = self.session_cache.write() {
            cache.set_active_session(cached);
        }

        let mut guard = self.session.write().map_err(|_| CoreError::LockPoisoned)?;
        *guard = Some(session);
        Ok(())
    }

    /// Clear session (logout/lock). Zeroes the key via Drop.
    pub fn clear_session(&self) -> Result<(), CoreError> {
        let mut guard = self.session.write().map_err(|_| CoreError::LockPoisoned)?;
        *guard = None;
        Ok(())
    }

    // ── Inactivity management ───────────────────────────────

    /// Check if the profile is locked (no active session).
    pub fn is_locked(&self) -> bool {
        self.session
            .read()
            .map(|guard| guard.is_none())
            .unwrap_or(true)
    }

    /// Lock the profile: drop the session (zeroes the key).
    /// Also clears all cached sessions (security: keys zeroed via Drop).
    pub fn lock(&self) {
        if let Ok(mut session) = self.session.write() {
            *session = None;
        }
        if let Ok(mut cache) = self.session_cache.write() {
            cache.clear();
        }
        tracing::info!("Profile locked, session cache cleared");
    }

    /// Update the last activity timestamp.
    pub fn update_activity(&self) {
        if let Ok(mut last) = self.last_activity.lock() {
            *last = Instant::now();
        }
    }

    /// Check if the inactivity timeout has been exceeded.
    ///
    /// Guards against large jumps from system sleep/suspend: Rust std docs state
    /// "It is not specified whether system suspends count as elapsed time or not."
    /// On WSL2, a host sleep can produce a massive elapsed() value. If elapsed
    /// exceeds timeout + 30 minutes, it's almost certainly a sleep/suspend — real
    /// inactivity would be caught within ~30s of timeout (the frontend poll interval).
    /// On jump detection, we reset the timer so the user gets a fresh window after wake.
    const SLEEP_JUMP_MARGIN_SECS: u64 = 1800; // 30 minutes beyond timeout

    pub fn check_timeout(&self) -> bool {
        self.last_activity
            .lock()
            .map(|mut last| {
                let elapsed = last.elapsed().as_secs();
                let timeout = self.inactivity_timeout_secs;
                if elapsed > timeout + Self::SLEEP_JUMP_MARGIN_SECS {
                    // Likely a sleep/suspend jump — reset timer, give user a fresh window
                    *last = Instant::now();
                    false
                } else {
                    elapsed > timeout
                }
            })
            .unwrap_or(false)
    }

    /// Minutes since last user activity (for batch extraction idle check).
    pub fn idle_minutes(&self) -> u64 {
        self.last_activity
            .lock()
            .map(|last| last.elapsed().as_secs() / 60)
            .unwrap_or(0)
    }

    // ── Audit logging ───────────────────────────────────────

    /// Log an access event. Auto-flushes to DB when buffer is full (IMP-002).
    pub fn log_access(&self, source: AccessSource, action: &str, entity: &str) {
        let needs_flush = self.audit.log(source, action, entity);
        if needs_flush {
            if let Err(e) = self.flush_and_prune_audit() {
                tracing::warn!("Auto-flush audit failed: {e}");
            }
        }
    }

    /// Get the current audit buffer contents (for testing/flush).
    pub fn audit_entries(&self) -> Vec<AuditEntry> {
        self.audit.entries()
    }

    /// Flush audit buffer to DB and prune entries older than 90 days.
    pub fn flush_and_prune_audit(&self) -> Result<(), CoreError> {
        let conn = self.open_db()?;
        self.audit.flush_to_db(&conn)?;
        if let Err(e) = crate::db::repository::prune_audit_log(&conn, 90) {
            tracing::warn!("Failed to prune audit log: {e}");
        }
        Ok(())
    }

    // ── Device manager (ME-02) ─────────────────────────────

    /// Load paired devices from the database into the DeviceManager.
    ///
    /// Call this after profile unlock when the DB is available.
    /// Restores which devices are paired across app restarts.
    /// MP-01: Tags loaded devices with the active profile's ID as owner.
    pub fn hydrate_devices(&self) -> Result<(), CoreError> {
        let owner_profile_id = {
            let guard = self.session.read().map_err(|_| CoreError::LockPoisoned)?;
            guard
                .as_ref()
                .map(|s| s.profile_id.to_string())
                .unwrap_or_default()
        };
        let conn = self.open_db()?;
        let loaded = DeviceManager::load_from_db(&conn, &owner_profile_id)
            .map_err(|e| CoreError::DeviceLoad(e.to_string()))?;
        let mut devices = self.devices.write().map_err(|_| CoreError::LockPoisoned)?;
        *devices = loaded;
        Ok(())
    }

    /// Read access to device manager.
    pub fn read_devices(
        &self,
    ) -> Result<RwLockReadGuard<'_, DeviceManager>, CoreError> {
        self.devices.read().map_err(|_| CoreError::LockPoisoned)
    }

    /// Write access to device manager.
    pub fn write_devices(
        &self,
    ) -> Result<RwLockWriteGuard<'_, DeviceManager>, CoreError> {
        self.devices.write().map_err(|_| CoreError::LockPoisoned)
    }

    // ── Session cache (MP-01) ──────────────────────────────

    /// Open a database connection for any cached profile.
    ///
    /// Unlike `open_db()` which only accesses the active session,
    /// this method can open any unlocked profile's database —
    /// used by companion device access via REST API.
    pub fn open_db_for_profile(
        &self,
        profile_id: &Uuid,
    ) -> Result<rusqlite::Connection, CoreError> {
        let cache = self
            .session_cache
            .read()
            .map_err(|_| CoreError::LockPoisoned)?;
        cache
            .open_db(profile_id)
            .map_err(CoreError::SessionCache)
    }

    /// Cache a profile session for companion access without switching
    /// the desktop UI. Caregiver unlocks managed profiles at startup.
    pub fn cache_profile_session(&self, session: CachedSession) -> Result<(), CoreError> {
        let mut cache = self
            .session_cache
            .write()
            .map_err(|_| CoreError::LockPoisoned)?;
        cache.cache_session(session);
        Ok(())
    }

    /// Evict a cached session (revoke companion access to a profile).
    /// Key is zeroed via CachedKey's Drop implementation.
    pub fn evict_cached_session(&self, profile_id: &Uuid) -> Result<(), CoreError> {
        let mut cache = self
            .session_cache
            .write()
            .map_err(|_| CoreError::LockPoisoned)?;
        cache.evict(profile_id);
        Ok(())
    }

    /// List all cached profile IDs (for companion access status).
    pub fn cached_profile_ids(&self) -> Result<Vec<Uuid>, CoreError> {
        let cache = self
            .session_cache
            .read()
            .map_err(|_| CoreError::LockPoisoned)?;
        Ok(cache.cached_profile_ids())
    }

    /// Check if a profile is unlocked in the session cache.
    pub fn is_profile_cached(&self, profile_id: &Uuid) -> bool {
        self.session_cache
            .read()
            .map(|cache| cache.is_unlocked(profile_id))
            .unwrap_or(false)
    }

    /// Get profile names for all cached sessions — used by companion access listing.
    pub fn cached_profile_names(&self) -> Result<Vec<(Uuid, String)>, CoreError> {
        let cache = self
            .session_cache
            .read()
            .map_err(|_| CoreError::LockPoisoned)?;
        Ok(cache
            .cached_profile_ids()
            .into_iter()
            .filter_map(|id| {
                cache
                    .get_session(&id)
                    .map(|s| (id, s.profile_name().to_string()))
            })
            .collect())
    }

    /// Open the app-level database (global device registry).
    pub fn open_app_db(&self) -> Result<rusqlite::Connection, CoreError> {
        db::app_db::open_app_database(&self.profiles_dir)
            .map_err(CoreError::Database)
    }

    // ── Pairing manager (M0-02) ─────────────────────────

    // ── Model resolver (L6-04) ────────────────────────────

    /// Access the shared model resolver.
    pub fn resolver(&self) -> &ActiveModelResolver {
        &self.model_resolver
    }

    // ── Ollama service ────────────────────────────────────

    /// Access the centralized Ollama service for exclusive SLM access.
    pub fn ollama(&self) -> &OllamaService {
        &self.ollama_service
    }

    /// S.1: Check if AI generation has been verified.
    pub fn is_ai_verified(&self) -> bool {
        self.ai_verified.load(Ordering::Relaxed)
    }

    /// S.1: Mark AI generation as verified (after successful test generation).
    pub fn set_ai_verified(&self, verified: bool) {
        self.ai_verified.store(verified, Ordering::Relaxed);
    }

    /// I18N-03: Get the user's preferred language from user_preferences.
    /// Returns "en" if no preference set or if profile is locked.
    pub fn get_profile_language(&self) -> String {
        self.open_db()
            .ok()
            .and_then(|conn| {
                crate::db::repository::get_user_preference(&conn, "language")
                    .ok()
                    .flatten()
            })
            .unwrap_or_else(|| "en".to_string())
    }

    /// Lock the pairing manager for exclusive access.
    pub fn lock_pairing(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, PairingManager>, CoreError> {
        self.pairing.lock().map_err(|_| CoreError::LockPoisoned)
    }
}

impl Default for CoreState {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════
// Error types
// ═══════════════════════════════════════════════════════════

/// Errors from CoreState operations.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("No active profile session")]
    NoActiveSession,
    #[error("Internal lock error")]
    LockPoisoned,
    #[error("Database error: {0}")]
    Database(#[from] db::DatabaseError),
    #[error("Session cache error: {0}")]
    SessionCache(#[from] SessionCacheError),
    #[error("Device load error: {0}")]
    DeviceLoad(String),
}

// ═══════════════════════════════════════════════════════════
// Access source tracking
// ═══════════════════════════════════════════════════════════

/// Identifies the source of a data access for audit logging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessSource {
    /// Access from the desktop Tauri UI.
    DesktopUi,
    /// Access from a paired mobile device.
    /// E8: `profile_id` tracks which profile's data was accessed.
    MobileDevice {
        device_id: String,
        profile_id: Option<String>,
    },
}

impl std::fmt::Display for AccessSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DesktopUi => write!(f, "desktop"),
            Self::MobileDevice { device_id, .. } => write!(f, "mobile:{device_id}"),
        }
    }
}

impl AccessSource {
    /// E8: Extract the profile_id if this is a mobile device access.
    pub fn profile_id(&self) -> Option<&str> {
        match self {
            Self::DesktopUi => None,
            Self::MobileDevice { profile_id, .. } => profile_id.as_deref(),
        }
    }
}

// ═══════════════════════════════════════════════════════════
// Audit logger
// ═══════════════════════════════════════════════════════════

/// In-memory audit log buffer. Entries are flushed to SQLite
/// when the buffer reaches capacity or on explicit flush.
pub struct AuditLogger {
    buffer: Mutex<Vec<AuditEntry>>,
}

/// A single audit log entry.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub source: AccessSource,
    pub action: String,
    pub entity: String,
    /// E8: Which profile's data was accessed (for medical compliance audit).
    pub profile_id: Option<String>,
}

impl AuditLogger {
    pub fn new() -> Self {
        Self {
            buffer: Mutex::new(Vec::with_capacity(AUDIT_BUFFER_CAPACITY)),
        }
    }

    /// Log an access event to the in-memory buffer.
    /// Returns `true` if the buffer has reached flush threshold.
    pub fn log(&self, source: AccessSource, action: &str, entity: &str) -> bool {
        if let Ok(mut buf) = self.buffer.lock() {
            let profile_id = source.profile_id().map(|s| s.to_string());
            buf.push(AuditEntry {
                timestamp: chrono::Utc::now(),
                source,
                action: action.to_string(),
                entity: entity.to_string(),
                profile_id,
            });
            buf.len() >= AUDIT_BUFFER_CAPACITY
        } else {
            false
        }
    }

    /// Get all buffered entries (for testing or manual flush).
    pub fn entries(&self) -> Vec<AuditEntry> {
        self.buffer
            .lock()
            .map(|buf| buf.clone())
            .unwrap_or_default()
    }

    /// Drain all buffered entries (for flush to SQLite).
    pub fn drain(&self) -> Vec<AuditEntry> {
        self.buffer
            .lock()
            .map(|mut buf| buf.drain(..).collect())
            .unwrap_or_default()
    }

    /// Current buffer size.
    pub fn buffer_len(&self) -> usize {
        self.buffer.lock().map(|buf| buf.len()).unwrap_or(0)
    }

    /// Flush buffered entries to SQLite and prune old entries (RS-ME-01-001).
    pub fn flush_to_db(&self, conn: &rusqlite::Connection) -> Result<usize, CoreError> {
        let entries = self.drain();
        if entries.is_empty() {
            return Ok(0);
        }

        let tuples: Vec<(String, String, String, String, Option<String>)> = entries
            .iter()
            .map(|e| {
                (
                    e.timestamp.to_rfc3339(),
                    e.source.to_string(),
                    e.action.clone(),
                    e.entity.clone(),
                    e.profile_id.clone(),
                )
            })
            .collect();

        let count = tuples.len();
        crate::db::repository::insert_audit_entries(conn, &tuples)?;

        tracing::debug!(count, "Flushed audit entries to database");
        Ok(count)
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_is_locked() {
        let state = CoreState::new();
        assert!(state.is_locked());
    }

    #[test]
    fn lock_on_already_locked_is_safe() {
        let state = CoreState::new();
        state.lock();
        assert!(state.is_locked());
    }

    #[test]
    fn update_activity_resets_timer() {
        let state = CoreState::new();
        state.update_activity();
        assert!(!state.check_timeout());
    }

    #[test]
    fn check_timeout_with_zero_threshold() {
        let state = CoreState {
            session: RwLock::new(None),
            session_cache: RwLock::new(SessionCache::new()),
            profiles_dir: PathBuf::from("/tmp"),
            inactivity_timeout_secs: 0,
            last_activity: Mutex::new(Instant::now() - std::time::Duration::from_secs(1)),
            transfer_server: tokio::sync::Mutex::new(None),
            distribution_server: tokio::sync::Mutex::new(None),
            api_server: tokio::sync::Mutex::new(None),
            devices: RwLock::new(DeviceManager::new()),
            pairing: Mutex::new(PairingManager::new()),
            audit: AuditLogger::new(),
            model_resolver: ActiveModelResolver::new(),
            ollama_service: OllamaService::new(),
            ai_verified: AtomicBool::new(false),
        };
        assert!(state.check_timeout());
    }

    #[test]
    fn open_db_fails_when_no_session() {
        let state = CoreState::new();
        let result = state.open_db();
        assert!(result.is_err());
        match result.unwrap_err() {
            CoreError::NoActiveSession => {}
            other => panic!("Expected NoActiveSession, got: {other}"),
        }
    }

    #[test]
    fn read_session_returns_none_when_locked() {
        let state = CoreState::new();
        let guard = state.read_session().unwrap();
        assert!(guard.is_none());
    }

    #[test]
    fn clear_session_on_empty_is_safe() {
        let state = CoreState::new();
        assert!(state.clear_session().is_ok());
        assert!(state.is_locked());
    }

    #[test]
    fn access_source_display() {
        assert_eq!(AccessSource::DesktopUi.to_string(), "desktop");
        assert_eq!(
            AccessSource::MobileDevice {
                device_id: "abc123".to_string(),
                profile_id: None,
            }
            .to_string(),
            "mobile:abc123"
        );
        // E8: Display ignores profile_id (used only for DB storage)
        assert_eq!(
            AccessSource::MobileDevice {
                device_id: "abc123".to_string(),
                profile_id: Some("prof-1".to_string()),
            }
            .to_string(),
            "mobile:abc123"
        );
    }

    #[test]
    fn device_manager_starts_empty() {
        let mgr = DeviceManager::new();
        assert_eq!(mgr.device_count(), 0);
        assert!(!mgr.is_paired("any-device"));
    }

    #[test]
    fn audit_logger_records_entries() {
        let logger = AuditLogger::new();
        assert_eq!(logger.buffer_len(), 0);

        logger.log(AccessSource::DesktopUi, "read_medications", "medications");
        assert_eq!(logger.buffer_len(), 1);

        let entries = logger.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, "read_medications");
        assert_eq!(entries[0].entity, "medications");
        assert_eq!(entries[0].source, AccessSource::DesktopUi);
    }

    #[test]
    fn audit_logger_drain_clears_buffer() {
        let logger = AuditLogger::new();
        logger.log(AccessSource::DesktopUi, "action1", "entity1");
        logger.log(AccessSource::DesktopUi, "action2", "entity2");
        assert_eq!(logger.buffer_len(), 2);

        let drained = logger.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(logger.buffer_len(), 0);
    }

    #[test]
    fn core_state_log_access() {
        let state = CoreState::new();
        state.log_access(
            AccessSource::MobileDevice {
                device_id: "phone-1".to_string(),
                profile_id: Some("prof-1".to_string()),
            },
            "read_home",
            "home_data",
        );
        let entries = state.audit_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source.to_string(), "mobile:phone-1");
        assert_eq!(entries[0].profile_id.as_deref(), Some("prof-1"));
    }

    #[test]
    fn concurrent_reads_do_not_block() {
        use std::sync::Arc;
        use std::thread;

        let state = Arc::new(CoreState::new());
        let mut handles = vec![];

        // Spawn 10 readers concurrently
        for _ in 0..10 {
            let state = Arc::clone(&state);
            handles.push(thread::spawn(move || {
                let guard = state.read_session().unwrap();
                assert!(guard.is_none());
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn core_error_display() {
        let err = CoreError::NoActiveSession;
        assert_eq!(err.to_string(), "No active profile session");

        let err = CoreError::LockPoisoned;
        assert_eq!(err.to_string(), "Internal lock error");
    }

    // --- Audit DB persistence (RS-ME-01-001) ---

    // --- IMP-002: Auto-flush threshold ---

    #[test]
    fn audit_log_returns_true_at_capacity() {
        let logger = AuditLogger::new();
        // Fill to just below capacity
        for i in 0..(AUDIT_BUFFER_CAPACITY - 1) {
            let needs_flush = logger.log(
                AccessSource::DesktopUi,
                &format!("action_{i}"),
                "entity",
            );
            assert!(!needs_flush, "Should not signal flush at {i}");
        }
        // The entry that hits capacity should return true
        let needs_flush = logger.log(AccessSource::DesktopUi, "action_final", "entity");
        assert!(needs_flush, "Should signal flush at capacity");
    }

    #[test]
    fn audit_flush_to_db_persists_entries() {
        use crate::db::sqlite::open_memory_database;

        let conn = open_memory_database().unwrap();
        let logger = AuditLogger::new();
        logger.log(AccessSource::DesktopUi, "read_meds", "medications");
        logger.log(
            AccessSource::MobileDevice {
                device_id: "phone-1".into(),
                profile_id: Some("prof-1".into()),
            },
            "read_home",
            "home_data",
        );

        let flushed = logger.flush_to_db(&conn).unwrap();
        assert_eq!(flushed, 2);
        assert_eq!(logger.buffer_len(), 0);

        // Verify entries in DB
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);

        // E8: Verify profile_id is persisted
        let profile_id: Option<String> = conn
            .query_row(
                "SELECT profile_id FROM audit_log WHERE source = 'mobile:phone-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(profile_id.as_deref(), Some("prof-1"));

        // Desktop entries have NULL profile_id
        let desktop_profile: Option<String> = conn
            .query_row(
                "SELECT profile_id FROM audit_log WHERE source = 'desktop'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(desktop_profile.is_none());
    }

    #[test]
    fn audit_flush_empty_buffer_is_noop() {
        use crate::db::sqlite::open_memory_database;

        let conn = open_memory_database().unwrap();
        let logger = AuditLogger::new();

        let flushed = logger.flush_to_db(&conn).unwrap();
        assert_eq!(flushed, 0);
    }

    #[test]
    fn audit_prune_removes_old_entries() {
        use crate::db::sqlite::open_memory_database;
        use crate::db::repository::prune_audit_log;

        let conn = open_memory_database().unwrap();

        // Insert an entry dated 100 days ago
        conn.execute(
            "INSERT INTO audit_log (timestamp, source, action, entity)
             VALUES (datetime('now', '-100 days'), 'desktop', 'old_action', 'old_entity')",
            [],
        )
        .unwrap();

        // Insert a recent entry
        conn.execute(
            "INSERT INTO audit_log (timestamp, source, action, entity)
             VALUES (datetime('now'), 'desktop', 'recent_action', 'recent_entity')",
            [],
        )
        .unwrap();

        let deleted = prune_audit_log(&conn, 90).unwrap();
        assert_eq!(deleted, 1);

        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining, 1);
    }

    #[test]
    fn ai_verified_flag_defaults_to_false() {
        let state = CoreState::new();
        assert!(!state.is_ai_verified());
    }

    #[test]
    fn ai_verified_flag_can_be_set_and_cleared() {
        let state = CoreState::new();
        state.set_ai_verified(true);
        assert!(state.is_ai_verified());
        state.set_ai_verified(false);
        assert!(!state.is_ai_verified());
    }

    // --- E8: Audit profile_id enrichment ---

    #[test]
    fn access_source_profile_id_extraction() {
        let desktop = AccessSource::DesktopUi;
        assert!(desktop.profile_id().is_none());

        let mobile_no_profile = AccessSource::MobileDevice {
            device_id: "dev-1".into(),
            profile_id: None,
        };
        assert!(mobile_no_profile.profile_id().is_none());

        let mobile_with_profile = AccessSource::MobileDevice {
            device_id: "dev-1".into(),
            profile_id: Some("prof-abc".into()),
        };
        assert_eq!(mobile_with_profile.profile_id(), Some("prof-abc"));
    }

    #[test]
    fn audit_entry_captures_profile_id_from_source() {
        let logger = AuditLogger::new();
        logger.log(
            AccessSource::MobileDevice {
                device_id: "dev-1".into(),
                profile_id: Some("prof-xyz".into()),
            },
            "read_meds",
            "medications",
        );
        let entries = logger.entries();
        assert_eq!(entries[0].profile_id.as_deref(), Some("prof-xyz"));
    }

    #[test]
    fn audit_entry_none_profile_for_desktop() {
        let logger = AuditLogger::new();
        logger.log(AccessSource::DesktopUi, "read_meds", "medications");
        let entries = logger.entries();
        assert!(entries[0].profile_id.is_none());
    }

    #[test]
    fn query_audit_by_profile_filters_correctly() {
        use crate::db::sqlite::open_memory_database;
        use crate::db::repository::query_audit_by_profile;

        let conn = open_memory_database().unwrap();
        let logger = AuditLogger::new();

        // Log entries for two different profiles
        logger.log(
            AccessSource::MobileDevice {
                device_id: "dev-1".into(),
                profile_id: Some("prof-A".into()),
            },
            "read_meds",
            "medications",
        );
        logger.log(
            AccessSource::MobileDevice {
                device_id: "dev-1".into(),
                profile_id: Some("prof-B".into()),
            },
            "read_home",
            "home_data",
        );
        logger.log(AccessSource::DesktopUi, "read_timeline", "timeline");

        logger.flush_to_db(&conn).unwrap();

        // Query for prof-A
        let results = query_audit_by_profile(&conn, "prof-A", 7).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].2, "read_meds"); // action

        // Query for prof-B
        let results = query_audit_by_profile(&conn, "prof-B", 7).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].2, "read_home");

        // Query for nonexistent profile
        let results = query_audit_by_profile(&conn, "prof-C", 7).unwrap();
        assert!(results.is_empty());
    }
}
