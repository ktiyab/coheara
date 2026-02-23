//! MP-01: Multi-profile session cache.
//!
//! Caches unlocked profile keys in memory (iOS Keychain pattern).
//! Enables companion phone access to multiple profiles without
//! switching the desktop UI's active profile.
//!
//! Key properties:
//! - Keys exist only in memory — never persisted to disk
//! - Keys zeroed via `Zeroize` trait on eviction or cache clear
//! - Inactivity timeout clears ALL sessions (security)
//! - App close clears ALL sessions

use std::collections::HashMap;
use std::path::PathBuf;

use uuid::Uuid;
use zeroize::Zeroize;

use crate::db;
use crate::db::DatabaseError;

// ═══════════════════════════════════════════════════════════
// CachedKey — zeroed on drop
// ═══════════════════════════════════════════════════════════

/// Cached encryption key — zeroed on drop to prevent memory leakage.
#[derive(Zeroize)]
#[zeroize(drop)]
struct CachedKey {
    bytes: [u8; 32],
}

impl CachedKey {
    fn new(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }
}

// ═══════════════════════════════════════════════════════════
// CachedSession — one unlocked profile
// ═══════════════════════════════════════════════════════════

/// A cached profile session — key + metadata for one unlocked profile.
pub struct CachedSession {
    profile_id: Uuid,
    profile_name: String,
    key: CachedKey,
    db_path: PathBuf,
}

impl CachedSession {
    /// Create a new cached session from raw components.
    pub fn new(profile_id: Uuid, profile_name: String, key_bytes: [u8; 32], db_path: PathBuf) -> Self {
        Self {
            profile_id,
            profile_name,
            key: CachedKey::new(key_bytes),
            db_path,
        }
    }

    pub fn profile_id(&self) -> Uuid {
        self.profile_id
    }

    pub fn profile_name(&self) -> &str {
        &self.profile_name
    }

    pub fn db_path(&self) -> &std::path::Path {
        &self.db_path
    }

    /// Open a database connection for this cached profile.
    pub fn open_db(&self) -> Result<rusqlite::Connection, DatabaseError> {
        db::open_database(&self.db_path, Some(&self.key.bytes))
    }
}

// ═══════════════════════════════════════════════════════════
// SessionCache — all unlocked profiles
// ═══════════════════════════════════════════════════════════

/// Multi-profile session cache.
///
/// Holds unlocked profile keys in memory so companion devices
/// can access multiple profiles without switching the desktop UI.
/// Pattern borrowed from iOS Keychain: keys in memory, zeroed on lock.
pub struct SessionCache {
    /// The profile currently shown on the desktop UI.
    active_id: Option<Uuid>,
    /// All unlocked profiles (including the active one).
    sessions: HashMap<Uuid, CachedSession>,
}

impl SessionCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            active_id: None,
            sessions: HashMap::new(),
        }
    }

    // ── Active session (desktop UI) ──────────────────────

    /// Get the active profile ID (currently shown on desktop).
    pub fn active_id(&self) -> Option<Uuid> {
        self.active_id
    }

    /// Get the active session (currently shown on desktop).
    pub fn active_session(&self) -> Option<&CachedSession> {
        self.active_id
            .and_then(|id| self.sessions.get(&id))
    }

    /// Set the active profile to an already-cached session.
    /// Returns false if the profile is not cached.
    pub fn set_active(&mut self, profile_id: Uuid) -> bool {
        if self.sessions.contains_key(&profile_id) {
            self.active_id = Some(profile_id);
            true
        } else {
            false
        }
    }

    // ── Cache operations ─────────────────────────────────

    /// Cache a session without switching the desktop UI.
    /// Used for companion access: unlock a managed profile in the background.
    pub fn cache_session(&mut self, session: CachedSession) {
        let id = session.profile_id;
        self.sessions.insert(id, session);
    }

    /// Cache a session AND set it as the active desktop profile.
    /// Used during normal desktop login/unlock.
    pub fn set_active_session(&mut self, session: CachedSession) {
        let id = session.profile_id;
        self.sessions.insert(id, session);
        self.active_id = Some(id);
    }

    /// Get a cached session by profile ID.
    pub fn get_session(&self, profile_id: &Uuid) -> Option<&CachedSession> {
        self.sessions.get(profile_id)
    }

    /// Check if a profile is unlocked (cached).
    pub fn is_unlocked(&self, profile_id: &Uuid) -> bool {
        self.sessions.contains_key(profile_id)
    }

    /// Evict a specific profile from the cache.
    /// Key is zeroed via CachedKey's Drop implementation.
    pub fn evict(&mut self, profile_id: &Uuid) {
        self.sessions.remove(profile_id);
        if self.active_id == Some(*profile_id) {
            self.active_id = None;
        }
    }

    /// Clear ALL cached sessions. Keys zeroed via Drop.
    /// Called on app close, inactivity timeout, or explicit lock.
    pub fn clear(&mut self) {
        self.sessions.clear();
        self.active_id = None;
    }

    /// List all cached profile IDs.
    pub fn cached_profile_ids(&self) -> Vec<Uuid> {
        self.sessions.keys().copied().collect()
    }

    /// Number of cached sessions.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Open a database connection for a cached profile.
    pub fn open_db(&self, profile_id: &Uuid) -> Result<rusqlite::Connection, SessionCacheError> {
        let session = self
            .sessions
            .get(profile_id)
            .ok_or(SessionCacheError::ProfileNotCached(*profile_id))?;
        session
            .open_db()
            .map_err(SessionCacheError::Database)
    }
}

impl Default for SessionCache {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════
// Error type
// ═══════════════════════════════════════════════════════════

/// Errors from session cache operations.
#[derive(Debug, thiserror::Error)]
pub enum SessionCacheError {
    #[error("Profile {0} is not cached (not unlocked)")]
    ProfileNotCached(Uuid),
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(id: Uuid, name: &str) -> CachedSession {
        CachedSession::new(
            id,
            name.to_string(),
            [0xAA; 32],
            PathBuf::from(format!("/tmp/test/{}/coheara.db", id)),
        )
    }

    #[test]
    fn new_cache_is_empty() {
        let cache = SessionCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert!(cache.active_id().is_none());
        assert!(cache.active_session().is_none());
    }

    #[test]
    fn cache_session_without_activating() {
        let mut cache = SessionCache::new();
        let id = Uuid::new_v4();

        cache.cache_session(make_session(id, "Alice"));

        assert_eq!(cache.len(), 1);
        assert!(cache.is_unlocked(&id));
        assert!(cache.active_id().is_none(), "Should not set active");
    }

    #[test]
    fn set_active_session_caches_and_activates() {
        let mut cache = SessionCache::new();
        let id = Uuid::new_v4();

        cache.set_active_session(make_session(id, "Alice"));

        assert_eq!(cache.len(), 1);
        assert!(cache.is_unlocked(&id));
        assert_eq!(cache.active_id(), Some(id));
        assert_eq!(cache.active_session().unwrap().profile_name(), "Alice");
    }

    #[test]
    fn set_active_to_cached_profile() {
        let mut cache = SessionCache::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        cache.set_active_session(make_session(id1, "Alice"));
        cache.cache_session(make_session(id2, "Bob"));

        assert_eq!(cache.active_id(), Some(id1));

        let switched = cache.set_active(id2);
        assert!(switched);
        assert_eq!(cache.active_id(), Some(id2));
    }

    #[test]
    fn set_active_to_uncached_profile_fails() {
        let mut cache = SessionCache::new();
        let id = Uuid::new_v4();
        let uncached = Uuid::new_v4();

        cache.set_active_session(make_session(id, "Alice"));

        let switched = cache.set_active(uncached);
        assert!(!switched);
        assert_eq!(cache.active_id(), Some(id), "Active unchanged");
    }

    #[test]
    fn get_session_returns_correct_profile() {
        let mut cache = SessionCache::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        cache.cache_session(make_session(id1, "Alice"));
        cache.cache_session(make_session(id2, "Bob"));

        let s1 = cache.get_session(&id1).unwrap();
        assert_eq!(s1.profile_name(), "Alice");

        let s2 = cache.get_session(&id2).unwrap();
        assert_eq!(s2.profile_name(), "Bob");

        let s3 = cache.get_session(&Uuid::new_v4());
        assert!(s3.is_none());
    }

    #[test]
    fn evict_removes_session_and_clears_active() {
        let mut cache = SessionCache::new();
        let id = Uuid::new_v4();

        cache.set_active_session(make_session(id, "Alice"));
        assert_eq!(cache.len(), 1);

        cache.evict(&id);
        assert_eq!(cache.len(), 0);
        assert!(cache.active_id().is_none());
        assert!(!cache.is_unlocked(&id));
    }

    #[test]
    fn evict_non_active_preserves_active() {
        let mut cache = SessionCache::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        cache.set_active_session(make_session(id1, "Alice"));
        cache.cache_session(make_session(id2, "Bob"));

        cache.evict(&id2);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.active_id(), Some(id1));
    }

    #[test]
    fn clear_removes_all_sessions() {
        let mut cache = SessionCache::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        cache.set_active_session(make_session(id1, "Alice"));
        cache.cache_session(make_session(id2, "Bob"));
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert!(cache.is_empty());
        assert!(cache.active_id().is_none());
    }

    #[test]
    fn cached_profile_ids_returns_all() {
        let mut cache = SessionCache::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        cache.cache_session(make_session(id1, "Alice"));
        cache.cache_session(make_session(id2, "Bob"));

        let ids = cache.cached_profile_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[test]
    fn open_db_for_uncached_profile_errors() {
        let cache = SessionCache::new();
        let id = Uuid::new_v4();

        let result = cache.open_db(&id);
        assert!(result.is_err());
        match result.unwrap_err() {
            SessionCacheError::ProfileNotCached(pid) => assert_eq!(pid, id),
            other => panic!("Expected ProfileNotCached, got: {other}"),
        }
    }

    #[test]
    fn replacing_session_drops_old_key() {
        let mut cache = SessionCache::new();
        let id = Uuid::new_v4();

        cache.cache_session(CachedSession::new(
            id,
            "Alice v1".to_string(),
            [0xAA; 32],
            PathBuf::from("/tmp/test/v1.db"),
        ));

        cache.cache_session(CachedSession::new(
            id,
            "Alice v2".to_string(),
            [0xBB; 32],
            PathBuf::from("/tmp/test/v2.db"),
        ));

        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get_session(&id).unwrap().profile_name(), "Alice v2");
    }
}
