//! M0-01: Shared types for the mobile API layer.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::core_state::CoreState;

/// Grace period for old tokens after rotation (30 seconds).
const TOKEN_GRACE_PERIOD_SECS: u64 = 30;

// ═══════════════════════════════════════════════════════════
// API context — shared state for the mobile API router
// ═══════════════════════════════════════════════════════════

/// Shared context for all API routes and middleware.
/// Wraps `CoreState` plus API-specific caches.
#[derive(Clone)]
pub struct ApiContext {
    pub core: Arc<CoreState>,
    pub nonce_cache: Arc<Mutex<NonceCache>>,
    pub rate_limiter: Arc<Mutex<RateLimiter>>,
    pub ws_tickets: Arc<Mutex<WsTicketStore>>,
}

impl ApiContext {
    pub fn new(core: Arc<CoreState>) -> Self {
        Self {
            core,
            nonce_cache: Arc::new(Mutex::new(NonceCache::new())),
            rate_limiter: Arc::new(Mutex::new(RateLimiter::new())),
            ws_tickets: Arc::new(Mutex::new(WsTicketStore::new())),
        }
    }
}

// ═══════════════════════════════════════════════════════════
// Device context — injected by auth middleware
// ═══════════════════════════════════════════════════════════

/// Authenticated device context, injected into request extensions
/// by the auth middleware after successful token validation.
#[derive(Debug, Clone)]
pub struct DeviceContext {
    pub device_id: String,
    pub device_name: String,
}

// ═══════════════════════════════════════════════════════════
// Token management for DeviceRegistry
// ═══════════════════════════════════════════════════════════

/// Token entry for a paired device. Supports rotation with grace period.
#[derive(Debug)]
pub struct TokenEntry {
    current_hash: [u8; 32],
    previous_hash: Option<[u8; 32]>,
    grace_expires: Option<Instant>,
}

impl TokenEntry {
    pub fn new(token_hash: [u8; 32]) -> Self {
        Self {
            current_hash: token_hash,
            previous_hash: None,
            grace_expires: None,
        }
    }

    /// Validate a token hash against current or grace-period previous.
    pub fn validate(&self, token_hash: &[u8; 32]) -> bool {
        if &self.current_hash == token_hash {
            return true;
        }
        if let (Some(prev), Some(exp)) = (&self.previous_hash, &self.grace_expires) {
            if prev == token_hash && Instant::now() < *exp {
                return true; // Within grace window
            }
        }
        false
    }

    /// Rotate to a new token, keeping the old one valid during grace period.
    pub fn rotate(&mut self, new_hash: [u8; 32]) {
        self.previous_hash = Some(self.current_hash);
        self.grace_expires =
            Some(Instant::now() + Duration::from_secs(TOKEN_GRACE_PERIOD_SECS));
        self.current_hash = new_hash;
    }
}

/// Hash a bearer token string using SHA-256.
pub fn hash_token(token: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hasher.finalize().into()
}

/// Generate a random bearer token (URL-safe base64, 32 bytes of entropy).
pub fn generate_token() -> String {
    use base64::Engine;
    let bytes: [u8; 32] = rand::random();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

// ═══════════════════════════════════════════════════════════
// Nonce cache — anti-replay protection
// ═══════════════════════════════════════════════════════════

/// In-memory nonce cache with TTL for anti-replay protection.
/// Nonces older than 60 seconds are cleaned up periodically.
pub struct NonceCache {
    seen: HashMap<String, Instant>,
    ttl: Duration,
}

impl NonceCache {
    pub fn new() -> Self {
        Self {
            seen: HashMap::new(),
            ttl: Duration::from_secs(60),
        }
    }

    /// Check if a nonce is fresh and insert it. Returns `true` if fresh.
    pub fn check_and_insert(&mut self, nonce: &str) -> bool {
        // Periodic cleanup when cache grows large
        if self.seen.len() > 1000 {
            self.cleanup();
        }

        if self.seen.contains_key(nonce) {
            return false; // Replay
        }

        self.seen.insert(nonce.to_string(), Instant::now());
        true
    }

    fn cleanup(&mut self) {
        let now = Instant::now();
        self.seen.retain(|_, ts| now.duration_since(*ts) < self.ttl);
    }
}

impl Default for NonceCache {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════
// Rate limiter — per-device sliding window
// ═══════════════════════════════════════════════════════════

/// Per-device rate limiter with per-minute and per-hour limits.
pub struct RateLimiter {
    windows: HashMap<String, Vec<Instant>>,
    per_minute: u32,
    per_hour: u32,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            per_minute: 100,
            per_hour: 1000,
        }
    }

    /// Check if a device is within rate limits. Returns `Ok(())` or
    /// `Err(retry_after_secs)` if exceeded.
    pub fn check(&mut self, device_id: &str) -> Result<(), u64> {
        let now = Instant::now();
        let entries = self.windows.entry(device_id.to_string()).or_default();

        // Clean entries older than 1 hour
        entries.retain(|ts| now.duration_since(*ts) < Duration::from_secs(3600));

        // Check per-minute
        let last_minute = entries
            .iter()
            .filter(|ts| now.duration_since(**ts) < Duration::from_secs(60))
            .count() as u32;
        if last_minute >= self.per_minute {
            return Err(60);
        }

        // Check per-hour
        if entries.len() as u32 >= self.per_hour {
            return Err(3600);
        }

        entries.push(now);
        Ok(())
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════
// WS ticket store — one-time WebSocket upgrade tokens (M0-03)
// ═══════════════════════════════════════════════════════════

/// One-time WebSocket upgrade ticket (30-second TTL).
/// Prevents session token exposure in WS query params.
struct WsTicket {
    device_id: String,
    device_name: String,
    expires_at: Instant,
}

/// Store for one-time WebSocket upgrade tickets.
pub struct WsTicketStore {
    tickets: HashMap<String, WsTicket>,
}

impl WsTicketStore {
    pub fn new() -> Self {
        Self {
            tickets: HashMap::new(),
        }
    }

    /// Issue a one-time ticket for the given device (30-second TTL).
    pub fn issue(&mut self, device_id: String, device_name: String) -> String {
        self.cleanup();
        let ticket = uuid::Uuid::new_v4().to_string();
        self.tickets.insert(
            ticket.clone(),
            WsTicket {
                device_id,
                device_name,
                expires_at: Instant::now() + Duration::from_secs(30),
            },
        );
        ticket
    }

    /// Consume a ticket (one-time use). Returns (device_id, device_name) on success.
    pub fn consume(&mut self, ticket: &str) -> Option<(String, String)> {
        let entry = self.tickets.remove(ticket)?;
        if Instant::now() > entry.expires_at {
            return None;
        }
        Some((entry.device_id, entry.device_name))
    }

    fn cleanup(&mut self) {
        let now = Instant::now();
        self.tickets.retain(|_, t| now < t.expires_at);
    }
}

impl Default for WsTicketStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_entry_validates_current() {
        let hash = hash_token("my-secret-token");
        let entry = TokenEntry::new(hash);
        assert!(entry.validate(&hash));
    }

    #[test]
    fn token_entry_rejects_wrong_hash() {
        let hash = hash_token("correct-token");
        let entry = TokenEntry::new(hash);
        let wrong = hash_token("wrong-token");
        assert!(!entry.validate(&wrong));
    }

    #[test]
    fn token_entry_grace_period() {
        let old_hash = hash_token("old-token");
        let new_hash = hash_token("new-token");
        let mut entry = TokenEntry::new(old_hash);

        entry.rotate(new_hash);

        // New token works
        assert!(entry.validate(&new_hash));
        // Old token still works (grace period)
        assert!(entry.validate(&old_hash));
    }

    #[test]
    fn token_entry_grace_period_expired() {
        let old_hash = hash_token("old-token");
        let new_hash = hash_token("new-token");
        let mut entry = TokenEntry::new(old_hash);

        // Manually set expired grace
        entry.previous_hash = Some(old_hash);
        entry.grace_expires = Some(Instant::now() - Duration::from_secs(1));
        entry.current_hash = new_hash;

        assert!(entry.validate(&new_hash));
        assert!(!entry.validate(&old_hash)); // Grace expired
    }

    #[test]
    fn generate_token_is_unique() {
        let t1 = generate_token();
        let t2 = generate_token();
        assert_ne!(t1, t2);
        assert!(!t1.is_empty());
    }

    #[test]
    fn hash_token_is_deterministic() {
        let h1 = hash_token("test");
        let h2 = hash_token("test");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_token_differs_for_different_inputs() {
        let h1 = hash_token("token-a");
        let h2 = hash_token("token-b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn nonce_cache_fresh_nonce_accepted() {
        let mut cache = NonceCache::new();
        assert!(cache.check_and_insert("nonce-1"));
    }

    #[test]
    fn nonce_cache_replay_rejected() {
        let mut cache = NonceCache::new();
        assert!(cache.check_and_insert("nonce-1"));
        assert!(!cache.check_and_insert("nonce-1")); // Replay
    }

    #[test]
    fn nonce_cache_different_nonces_accepted() {
        let mut cache = NonceCache::new();
        assert!(cache.check_and_insert("nonce-1"));
        assert!(cache.check_and_insert("nonce-2"));
    }

    #[test]
    fn rate_limiter_allows_under_limit() {
        let mut limiter = RateLimiter::new();
        assert!(limiter.check("device-1").is_ok());
        assert!(limiter.check("device-1").is_ok());
    }

    #[test]
    fn rate_limiter_rejects_over_per_minute() {
        let mut limiter = RateLimiter {
            windows: HashMap::new(),
            per_minute: 2,
            per_hour: 1000,
        };
        assert!(limiter.check("device-1").is_ok());
        assert!(limiter.check("device-1").is_ok());
        assert_eq!(limiter.check("device-1"), Err(60));
    }

    #[test]
    fn rate_limiter_isolates_devices() {
        let mut limiter = RateLimiter {
            windows: HashMap::new(),
            per_minute: 1,
            per_hour: 1000,
        };
        assert!(limiter.check("device-1").is_ok());
        assert!(limiter.check("device-2").is_ok()); // Different device, OK
        assert_eq!(limiter.check("device-1"), Err(60)); // Same device, blocked
    }

    // ── WsTicketStore tests (M0-03) ──────────────────────────

    #[test]
    fn ws_ticket_issue_returns_unique() {
        let mut store = WsTicketStore::new();
        let t1 = store.issue("dev-1".into(), "Phone 1".into());
        let t2 = store.issue("dev-2".into(), "Phone 2".into());
        assert_ne!(t1, t2);
        assert!(!t1.is_empty());
    }

    #[test]
    fn ws_ticket_consume_valid() {
        let mut store = WsTicketStore::new();
        let ticket = store.issue("dev-1".into(), "Phone 1".into());
        let result = store.consume(&ticket);
        assert!(result.is_some());
        let (device_id, device_name) = result.unwrap();
        assert_eq!(device_id, "dev-1");
        assert_eq!(device_name, "Phone 1");
    }

    #[test]
    fn ws_ticket_consume_already_used() {
        let mut store = WsTicketStore::new();
        let ticket = store.issue("dev-1".into(), "Phone 1".into());
        let _ = store.consume(&ticket);
        let result = store.consume(&ticket);
        assert!(result.is_none());
    }

    #[test]
    fn ws_ticket_consume_invalid() {
        let mut store = WsTicketStore::new();
        let result = store.consume("nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn ws_ticket_consume_expired() {
        let mut store = WsTicketStore::new();
        store.tickets.insert(
            "expired-ticket".to_string(),
            WsTicket {
                device_id: "dev-1".into(),
                device_name: "Phone 1".into(),
                expires_at: Instant::now() - Duration::from_secs(1),
            },
        );
        let result = store.consume("expired-ticket");
        assert!(result.is_none());
    }
}
