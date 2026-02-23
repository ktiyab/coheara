//! M0-02: Device Pairing Protocol.
//!
//! Implements the complete pairing flow:
//! 1. Desktop generates QR code (pairing token + ECDH pubkey + cert fingerprint)
//! 2. Phone scans QR and POSTs to /api/auth/pair
//! 3. Desktop shows confirmation dialog
//! 4. User approves → ECDH key exchange → session token issued
//!
//! Security: X25519 ECDH key exchange, HKDF-SHA256 key derivation,
//! one-time pairing tokens (5-min expiry), per-request token rotation.

use std::time::{Duration, Instant};

use base64::Engine;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use tokio::sync::oneshot;

use crate::api::types::{generate_token, hash_token};

/// Pairing token lifetime (5 minutes).
const PAIRING_TOKEN_TTL_SECS: u64 = 300;

/// Timeout waiting for desktop user approval (60 seconds).
const APPROVAL_TIMEOUT_SECS: u64 = 60;

// ═══════════════════════════════════════════════════════════
// Error type
// ═══════════════════════════════════════════════════════════

/// Errors from the pairing protocol.
#[derive(Debug, thiserror::Error)]
pub enum PairingError {
    #[error("No active pairing session")]
    NoPairingActive,
    #[error("Pairing token expired")]
    TokenExpired,
    #[error("Pairing token invalid")]
    TokenInvalid,
    #[error("Pairing token already consumed")]
    TokenConsumed,
    #[error("A pairing attempt is already in progress")]
    AlreadyInProgress,
    #[error("Desktop user denied the pairing request")]
    Denied,
    #[error("Approval timed out")]
    ApprovalTimeout,
    #[error("Invalid phone public key")]
    InvalidPublicKey,
    #[error("Key derivation failed")]
    KeyDerivation,
    #[error("Maximum devices reached")]
    MaxDevices,
    #[error("No pending approval")]
    NoPendingApproval,
}

// ═══════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════

/// QR code content for the phone to scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrPairingData {
    /// Protocol version.
    pub v: u8,
    /// Desktop API server URL.
    pub url: String,
    /// One-time pairing token (base64).
    pub token: String,
    /// TLS certificate fingerprint (SHA-256, colon-separated hex).
    pub cert_fp: String,
    /// Desktop X25519 public key (base64).
    pub pubkey: String,
}

/// Response to the desktop UI after starting pairing.
#[derive(Debug, Clone, Serialize)]
pub struct PairingStartResponse {
    /// QR code as SVG string.
    pub qr_svg: String,
    /// QR code JSON content (for display).
    pub qr_data: QrPairingData,
    /// When the pairing token expires (ISO 8601).
    pub expires_at: String,
}

/// Phone's pairing request body.
#[derive(Debug, Clone, Deserialize)]
pub struct PairRequest {
    /// Pairing token from QR code.
    pub token: String,
    /// Phone's X25519 public key (base64, 32 bytes).
    pub phone_pubkey: String,
    /// Human-readable device name.
    pub device_name: String,
    /// Device model identifier.
    pub device_model: String,
}

/// Data about a pending pairing request awaiting desktop approval.
#[derive(Debug, Clone, Serialize)]
pub struct PendingApproval {
    /// Device name from the phone.
    pub device_name: String,
    /// Device model from the phone.
    pub device_model: String,
}

/// Successful pairing response sent back to the phone.
#[derive(Debug, Clone, Serialize)]
pub struct PairResponse {
    /// Bearer token for subsequent API calls.
    pub session_token: String,
    /// Cache encryption key, encrypted with the shared secret (base64).
    pub cache_key_encrypted: String,
    /// Active profile name on the desktop.
    pub profile_name: String,
    /// MP-01: Profiles this device can access (own + managed).
    pub accessible_profiles: Vec<AccessibleProfile>,
}

/// MP-01: A profile accessible to a paired device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibleProfile {
    pub profile_id: String,
    pub profile_name: String,
    /// "own", "managed", or "granted"
    pub relationship: String,
    pub color_index: Option<u8>,
}

// ═══════════════════════════════════════════════════════════
// Active pairing session (held in memory)
// ═══════════════════════════════════════════════════════════

/// An active pairing session on the desktop side.
#[allow(dead_code)]
struct ActivePairing {
    /// The one-time pairing token (plaintext for validation).
    token: String,
    /// When the token was created.
    created_at: Instant,
    /// Desktop's X25519 static secret (persisted for ECDH).
    desktop_secret: x25519_dalek::StaticSecret,
    /// Desktop's X25519 public key (retained for audit/debugging).
    desktop_public: x25519_dalek::PublicKey,
    /// TLS certificate fingerprint included in QR (retained for audit).
    cert_fingerprint: String,
    /// Server URL included in QR (retained for audit).
    server_url: String,
    /// Whether the token has been consumed by a phone request.
    consumed: bool,
}

/// A pairing request waiting for desktop user approval.
struct PendingPairRequest {
    /// Phone's info.
    device_name: String,
    device_model: String,
    /// Phone's X25519 public key bytes.
    phone_public_bytes: [u8; 32],
    /// Channel to send approval/denial result.
    /// `None` after `signal_approval()` has been called.
    response_tx: Option<oneshot::Sender<bool>>,
}

// ═══════════════════════════════════════════════════════════
// PairingManager
// ═══════════════════════════════════════════════════════════

/// Manages the pairing flow between desktop and phone.
///
/// At most one pairing session can be active at a time.
/// Lives in `CoreState` behind a `Mutex`.
pub struct PairingManager {
    /// Currently active pairing session (if any).
    active: Option<ActivePairing>,
    /// Pending approval request (phone submitted, waiting for desktop user).
    pending: Option<PendingPairRequest>,
}

impl Default for PairingManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PairingManager {
    pub fn new() -> Self {
        Self {
            active: None,
            pending: None,
        }
    }

    /// Start a new pairing session.
    ///
    /// Generates an X25519 keypair and a one-time pairing token.
    /// Returns the QR data for display.
    pub fn start(
        &mut self,
        server_url: String,
        cert_fingerprint: String,
    ) -> Result<PairingStartResponse, PairingError> {
        // Clean up expired sessions
        self.cleanup_expired();

        if self.active.is_some() {
            return Err(PairingError::AlreadyInProgress);
        }

        let desktop_secret = x25519_dalek::StaticSecret::random_from_rng(rand::thread_rng());
        let desktop_public = x25519_dalek::PublicKey::from(&desktop_secret);
        let token = generate_pairing_token();

        let qr_data = QrPairingData {
            v: 1,
            url: server_url.clone(),
            token: token.clone(),
            cert_fp: cert_fingerprint.clone(),
            pubkey: base64::engine::general_purpose::STANDARD.encode(desktop_public.as_bytes()),
        };

        let qr_json =
            serde_json::to_string(&qr_data).map_err(|_| PairingError::KeyDerivation)?;
        let qr_svg = crate::wifi_transfer::generate_qr_code(&qr_json)
            .map_err(|_| PairingError::KeyDerivation)?;

        let expires_at =
            chrono::Utc::now() + chrono::Duration::seconds(PAIRING_TOKEN_TTL_SECS as i64);

        self.active = Some(ActivePairing {
            token,
            created_at: Instant::now(),
            desktop_secret,
            desktop_public,
            cert_fingerprint,
            server_url,
            consumed: false,
        });

        Ok(PairingStartResponse {
            qr_svg,
            qr_data,
            expires_at: expires_at.to_rfc3339(),
        })
    }

    /// Cancel the active pairing session.
    pub fn cancel(&mut self) {
        self.active = None;
        // Drop pending request — the oneshot will be dropped, causing a RecvError
        self.pending = None;
    }

    /// Get the active QR pairing data if a session is active and not expired.
    ///
    /// Used by the distribution server's `/pairing-info` endpoint to let
    /// PWA companions auto-pair without scanning a JSON QR code.
    pub fn active_qr_data(&self) -> Option<QrPairingData> {
        let session = self.active.as_ref()?;
        // Check expiry
        if session.created_at.elapsed() > Duration::from_secs(PAIRING_TOKEN_TTL_SECS) {
            return None;
        }
        // Don't expose if token was already consumed by a phone request
        if session.consumed {
            return None;
        }
        Some(QrPairingData {
            v: 1,
            url: session.server_url.clone(),
            token: session.token.clone(),
            cert_fp: session.cert_fingerprint.clone(),
            pubkey: base64::engine::general_purpose::STANDARD
                .encode(session.desktop_public.as_bytes()),
        })
    }

    /// Submit a pairing request from the phone.
    ///
    /// Validates the token, stores the phone's info, and returns a
    /// `oneshot::Receiver` that the phone handler should await for
    /// the desktop user's approval decision.
    pub fn submit_pair_request(
        &mut self,
        request: &PairRequest,
    ) -> Result<oneshot::Receiver<bool>, PairingError> {
        let session = self.active.as_mut().ok_or(PairingError::NoPairingActive)?;

        // Check token validity
        if session.consumed {
            return Err(PairingError::TokenConsumed);
        }
        // Constant-time comparison to prevent timing attacks (RS-M002-03)
        let stored_hash = hash_token(&session.token);
        let request_hash = hash_token(&request.token);
        if stored_hash.ct_eq(&request_hash).unwrap_u8() == 0 {
            return Err(PairingError::TokenInvalid);
        }
        if session.created_at.elapsed() > Duration::from_secs(PAIRING_TOKEN_TTL_SECS) {
            self.active = None;
            return Err(PairingError::TokenExpired);
        }

        // Decode phone's public key
        let pubkey_bytes = base64::engine::general_purpose::STANDARD
            .decode(&request.phone_pubkey)
            .map_err(|_| PairingError::InvalidPublicKey)?;
        if pubkey_bytes.len() != 32 {
            return Err(PairingError::InvalidPublicKey);
        }
        let mut phone_public: [u8; 32] = [0u8; 32];
        phone_public.copy_from_slice(&pubkey_bytes);

        // Mark token as consumed (one-time use)
        session.consumed = true;

        // Create approval channel
        let (tx, rx) = oneshot::channel();

        self.pending = Some(PendingPairRequest {
            device_name: request.device_name.clone(),
            device_model: request.device_model.clone(),
            phone_public_bytes: phone_public,
            response_tx: Some(tx),
        });

        Ok(rx)
    }

    /// Get info about the pending pairing request (for desktop UI).
    pub fn pending_approval(&self) -> Option<PendingApproval> {
        self.pending.as_ref().map(|p| PendingApproval {
            device_name: p.device_name.clone(),
            device_model: p.device_model.clone(),
        })
    }

    /// Signal approval to the waiting phone HTTP handler.
    ///
    /// Sends `true` on the oneshot channel so the phone's long-poll unblocks.
    /// Does NOT consume the pairing session — call `complete_pairing()` next
    /// to perform ECDH and get the session token.
    ///
    /// This is called by the **desktop IPC** command. The **HTTP endpoint**
    /// then calls `complete_pairing()` after receiving the signal.
    pub fn signal_approval(&mut self) -> Result<(), PairingError> {
        let pending = self.pending.as_mut().ok_or(PairingError::NoPendingApproval)?;
        let tx = pending
            .response_tx
            .take()
            .ok_or(PairingError::NoPendingApproval)?;
        let _ = tx.send(true);
        Ok(())
    }

    /// Complete the pairing after approval has been signaled.
    ///
    /// Performs ECDH key exchange, derives the cache key, generates a session
    /// token, and returns all data needed to register the device.
    ///
    /// This is called by the **HTTP endpoint** after the phone receives the
    /// approval signal and the handler unblocks.
    pub fn complete_pairing(&mut self) -> Result<ApprovedPairing, PairingError> {
        let session = self.active.take().ok_or(PairingError::NoPairingActive)?;
        let pending = self.pending.take().ok_or(PairingError::NoPendingApproval)?;

        // ECDH key exchange
        let phone_public = x25519_dalek::PublicKey::from(pending.phone_public_bytes);
        let shared_secret = session.desktop_secret.diffie_hellman(&phone_public);

        // Derive cache encryption key via HKDF-SHA256
        let cache_key = derive_cache_key(shared_secret.as_bytes())?;

        // Generate initial session token
        let session_token = generate_token();
        let token_hash = hash_token(&session_token);

        Ok(ApprovedPairing {
            device_name: pending.device_name,
            device_model: pending.device_model,
            phone_public_key: pending.phone_public_bytes,
            session_token,
            token_hash,
            cache_key,
        })
    }

    /// Convenience: signal approval AND complete in one call.
    ///
    /// For callers that own both the desktop and phone paths (e.g., tests).
    /// In production, use `signal_approval()` + `complete_pairing()` separately.
    pub fn approve(&mut self) -> Result<ApprovedPairing, PairingError> {
        self.signal_approval()?;
        self.complete_pairing()
    }

    /// Deny the pending pairing request.
    pub fn deny(&mut self) {
        if let Some(mut pending) = self.pending.take() {
            if let Some(tx) = pending.response_tx.take() {
                let _ = tx.send(false);
            }
        }
        self.active = None;
    }

    /// Check if a pairing session is active.
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// Check if there's a pending approval.
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Clean up expired pairing sessions.
    fn cleanup_expired(&mut self) {
        if let Some(ref session) = self.active {
            if session.created_at.elapsed() > Duration::from_secs(PAIRING_TOKEN_TTL_SECS) {
                self.active = None;
                self.pending = None;
            }
        }
    }
}

// Explicit Debug implementation (StaticSecret is not Debug)
impl std::fmt::Debug for PairingManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PairingManager")
            .field("active", &self.active.is_some())
            .field("pending", &self.pending.is_some())
            .finish()
    }
}

/// Data from an approved pairing, needed to register the device.
pub struct ApprovedPairing {
    pub device_name: String,
    pub device_model: String,
    pub phone_public_key: [u8; 32],
    pub session_token: String,
    pub token_hash: [u8; 32],
    pub cache_key: [u8; 32],
}

/// Approval timeout duration.
pub fn approval_timeout() -> Duration {
    Duration::from_secs(APPROVAL_TIMEOUT_SECS)
}

// ═══════════════════════════════════════════════════════════
// Crypto helpers
// ═══════════════════════════════════════════════════════════

/// Generate a one-time pairing token (base64, 32 bytes of entropy).
fn generate_pairing_token() -> String {
    let bytes: [u8; 32] = rand::random();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Derive a cache encryption key from the ECDH shared secret.
///
/// Uses HKDF-SHA256 with a fixed salt and info string.
fn derive_cache_key(shared_secret: &[u8]) -> Result<[u8; 32], PairingError> {
    use hkdf::Hkdf;
    use sha2::Sha256;

    let hk = Hkdf::<Sha256>::new(Some(b"coheara-cache-key"), shared_secret);
    let mut key = [0u8; 32];
    hk.expand(b"v1", &mut key)
        .map_err(|_| PairingError::KeyDerivation)?;
    Ok(key)
}

/// Encrypt the cache key for transport to the phone.
///
/// Uses AES-256-GCM with the shared secret as the encryption key.
/// The phone derives the same shared secret and can decrypt.
pub fn encrypt_cache_key_for_transport(
    cache_key: &[u8; 32],
    shared_secret_bytes: &[u8],
) -> Result<String, PairingError> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    // Derive a transport key from the shared secret
    let transport_key = derive_transport_key(shared_secret_bytes)?;

    let nonce_bytes: [u8; 12] = rand::random();
    let cipher = Aes256Gcm::new((&transport_key).into());
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, cache_key.as_slice())
        .map_err(|_| PairingError::KeyDerivation)?;

    // Combine nonce + ciphertext → base64
    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    Ok(base64::engine::general_purpose::STANDARD.encode(&combined))
}

/// Derive a transport encryption key from the shared secret (for cache_key_encrypted).
fn derive_transport_key(shared_secret: &[u8]) -> Result<[u8; 32], PairingError> {
    use hkdf::Hkdf;
    use sha2::Sha256;

    let hk = Hkdf::<Sha256>::new(Some(b"coheara-transport-key"), shared_secret);
    let mut key = [0u8; 32];
    hk.expand(b"v1", &mut key)
        .map_err(|_| PairingError::KeyDerivation)?;
    Ok(key)
}

// ═══════════════════════════════════════════════════════════
// Database persistence for paired devices
// ═══════════════════════════════════════════════════════════

/// Store a paired device in the database.
pub fn db_store_paired_device(
    conn: &rusqlite::Connection,
    device_id: &str,
    device_name: &str,
    device_model: &str,
    public_key: &[u8; 32],
) -> Result<(), rusqlite::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO paired_devices (device_id, device_name, device_model, public_key, paired_at, last_seen, is_revoked)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
        rusqlite::params![device_id, device_name, device_model, public_key.as_slice(), now, now],
    )?;
    Ok(())
}

/// Store a device session in the database.
pub fn db_store_session(
    conn: &rusqlite::Connection,
    device_id: &str,
    token_hash: &[u8; 32],
) -> Result<(), rusqlite::Error> {
    let now = chrono::Utc::now();
    let expires = now + chrono::Duration::hours(24);
    conn.execute(
        "INSERT INTO device_sessions (device_id, token_hash, created_at, expires_at, last_used)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            device_id,
            token_hash.as_slice(),
            now.to_rfc3339(),
            expires.to_rfc3339(),
            now.to_rfc3339(),
        ],
    )?;
    Ok(())
}

/// Revoke a device: mark as revoked and delete sessions.
pub fn db_revoke_device(
    conn: &rusqlite::Connection,
    device_id: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE paired_devices SET is_revoked = 1 WHERE device_id = ?1",
        rusqlite::params![device_id],
    )?;
    conn.execute(
        "DELETE FROM device_sessions WHERE device_id = ?1",
        rusqlite::params![device_id],
    )?;
    Ok(())
}

/// Load all paired devices from the database (for hydrating DeviceManager on startup).
pub fn db_load_paired_devices(
    conn: &rusqlite::Connection,
) -> Result<Vec<StoredDevice>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT device_id, device_name, device_model, public_key, paired_at, last_seen, is_revoked
         FROM paired_devices",
    )?;
    let rows = stmt.query_map([], |row| {
        let pk_bytes: Vec<u8> = row.get(3)?;
        let mut public_key = [0u8; 32];
        if pk_bytes.len() == 32 {
            public_key.copy_from_slice(&pk_bytes);
        }
        Ok(StoredDevice {
            device_id: row.get(0)?,
            device_name: row.get(1)?,
            device_model: row.get(2)?,
            public_key,
            paired_at: row.get(4)?,
            last_seen: row.get(5)?,
            is_revoked: row.get::<_, i32>(6)? != 0,
        })
    })?;
    rows.collect()
}

/// A device loaded from the database.
#[derive(Debug, Clone)]
pub struct StoredDevice {
    pub device_id: String,
    pub device_name: String,
    pub device_model: String,
    pub public_key: [u8; 32],
    pub paired_at: String,
    pub last_seen: String,
    pub is_revoked: bool,
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Pairing token tests ───────────────────────────────

    #[test]
    fn pairing_token_is_base64_32_bytes() {
        let token = generate_pairing_token();
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&token)
            .unwrap();
        assert_eq!(decoded.len(), 32);
    }

    #[test]
    fn pairing_tokens_are_unique() {
        let t1 = generate_pairing_token();
        let t2 = generate_pairing_token();
        assert_ne!(t1, t2);
    }

    // ── ECDH + HKDF tests ────────────────────────────────

    #[test]
    fn ecdh_produces_same_shared_secret() {
        let desktop_secret = x25519_dalek::StaticSecret::random_from_rng(rand::thread_rng());
        let desktop_public = x25519_dalek::PublicKey::from(&desktop_secret);

        let phone_secret = x25519_dalek::StaticSecret::random_from_rng(rand::thread_rng());
        let phone_public = x25519_dalek::PublicKey::from(&phone_secret);

        let desktop_shared = desktop_secret.diffie_hellman(&phone_public);
        let phone_shared = phone_secret.diffie_hellman(&desktop_public);

        assert_eq!(desktop_shared.as_bytes(), phone_shared.as_bytes());
    }

    #[test]
    fn hkdf_derives_deterministic_key() {
        let secret = [42u8; 32];
        let key1 = derive_cache_key(&secret).unwrap();
        let key2 = derive_cache_key(&secret).unwrap();
        assert_eq!(key1, key2);
    }

    #[test]
    fn hkdf_different_secrets_produce_different_keys() {
        let key1 = derive_cache_key(&[1u8; 32]).unwrap();
        let key2 = derive_cache_key(&[2u8; 32]).unwrap();
        assert_ne!(key1, key2);
    }

    #[test]
    fn cache_key_encryption_roundtrip() {
        let cache_key: [u8; 32] = rand::random();
        let shared_secret = [42u8; 32];

        let encrypted = encrypt_cache_key_for_transport(&cache_key, &shared_secret).unwrap();
        assert!(!encrypted.is_empty());

        // Decrypt manually
        let transport_key = derive_transport_key(&shared_secret).unwrap();
        let combined = base64::engine::general_purpose::STANDARD
            .decode(&encrypted)
            .unwrap();
        let (nonce_bytes, ciphertext) = combined.split_at(12);

        use aes_gcm::aead::{Aead, KeyInit};
        use aes_gcm::{Aes256Gcm, Nonce};
        let cipher = Aes256Gcm::new((&transport_key).into());
        let decrypted = cipher
            .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
            .unwrap();
        assert_eq!(decrypted, cache_key);
    }

    // ── PairingManager tests ──────────────────────────────

    #[test]
    fn start_pairing_creates_session() {
        let mut mgr = PairingManager::new();
        let result = mgr.start("https://192.168.1.42:8443".into(), "SHA256:AB:CD".into());
        assert!(result.is_ok());
        assert!(mgr.is_active());
    }

    #[test]
    fn start_pairing_twice_errors() {
        let mut mgr = PairingManager::new();
        mgr.start("https://192.168.1.42:8443".into(), "fp".into())
            .unwrap();
        let result = mgr.start("https://192.168.1.42:8443".into(), "fp".into());
        assert!(matches!(result, Err(PairingError::AlreadyInProgress)));
    }

    #[test]
    fn cancel_clears_session() {
        let mut mgr = PairingManager::new();
        mgr.start("https://192.168.1.42:8443".into(), "fp".into())
            .unwrap();
        assert!(mgr.is_active());

        mgr.cancel();
        assert!(!mgr.is_active());
    }

    #[test]
    fn submit_with_wrong_token_fails() {
        let mut mgr = PairingManager::new();
        mgr.start("https://192.168.1.42:8443".into(), "fp".into())
            .unwrap();

        let phone_secret = x25519_dalek::StaticSecret::random_from_rng(rand::thread_rng());
        let phone_public = x25519_dalek::PublicKey::from(&phone_secret);

        let request = PairRequest {
            token: "wrong-token".into(),
            phone_pubkey: base64::engine::general_purpose::STANDARD
                .encode(phone_public.as_bytes()),
            device_name: "Test Phone".into(),
            device_model: "TestModel".into(),
        };

        let result = mgr.submit_pair_request(&request);
        assert!(matches!(result, Err(PairingError::TokenInvalid)));
    }

    #[test]
    fn submit_with_valid_token_succeeds() {
        let mut mgr = PairingManager::new();
        let start = mgr
            .start("https://192.168.1.42:8443".into(), "fp".into())
            .unwrap();

        let phone_secret = x25519_dalek::StaticSecret::random_from_rng(rand::thread_rng());
        let phone_public = x25519_dalek::PublicKey::from(&phone_secret);

        let request = PairRequest {
            token: start.qr_data.token.clone(),
            phone_pubkey: base64::engine::general_purpose::STANDARD
                .encode(phone_public.as_bytes()),
            device_name: "Test Phone".into(),
            device_model: "TestModel".into(),
        };

        let result = mgr.submit_pair_request(&request);
        assert!(result.is_ok());
        assert!(mgr.has_pending());
    }

    #[test]
    fn token_is_one_time_use() {
        let mut mgr = PairingManager::new();
        let start = mgr
            .start("https://192.168.1.42:8443".into(), "fp".into())
            .unwrap();

        let phone_secret = x25519_dalek::StaticSecret::random_from_rng(rand::thread_rng());
        let phone_public = x25519_dalek::PublicKey::from(&phone_secret);

        let request = PairRequest {
            token: start.qr_data.token.clone(),
            phone_pubkey: base64::engine::general_purpose::STANDARD
                .encode(phone_public.as_bytes()),
            device_name: "Test Phone".into(),
            device_model: "TestModel".into(),
        };

        // First submit succeeds
        let _ = mgr.submit_pair_request(&request).unwrap();

        // Second submit fails (token consumed)
        let result = mgr.submit_pair_request(&request);
        assert!(matches!(result, Err(PairingError::TokenConsumed)));
    }

    #[test]
    fn approve_completes_ecdh() {
        let mut mgr = PairingManager::new();
        let start = mgr
            .start("https://192.168.1.42:8443".into(), "fp".into())
            .unwrap();

        let phone_secret = x25519_dalek::StaticSecret::random_from_rng(rand::thread_rng());
        let phone_public = x25519_dalek::PublicKey::from(&phone_secret);

        let request = PairRequest {
            token: start.qr_data.token.clone(),
            phone_pubkey: base64::engine::general_purpose::STANDARD
                .encode(phone_public.as_bytes()),
            device_name: "Test Phone".into(),
            device_model: "TestModel".into(),
        };

        let _rx = mgr.submit_pair_request(&request).unwrap();
        let approved = mgr.approve().unwrap();

        assert_eq!(approved.device_name, "Test Phone");
        assert_eq!(approved.device_model, "TestModel");
        assert!(!approved.session_token.is_empty());
        assert_ne!(approved.cache_key, [0u8; 32]);

        // Verify phone can derive the same cache key
        let desktop_pubkey_bytes = base64::engine::general_purpose::STANDARD
            .decode(&start.qr_data.pubkey)
            .unwrap();
        let mut desktop_pub_arr = [0u8; 32];
        desktop_pub_arr.copy_from_slice(&desktop_pubkey_bytes);
        let desktop_public = x25519_dalek::PublicKey::from(desktop_pub_arr);
        let phone_shared = phone_secret.diffie_hellman(&desktop_public);
        let phone_cache_key = derive_cache_key(phone_shared.as_bytes()).unwrap();
        assert_eq!(phone_cache_key, approved.cache_key);
    }

    #[test]
    fn deny_clears_state() {
        let mut mgr = PairingManager::new();
        let start = mgr
            .start("https://192.168.1.42:8443".into(), "fp".into())
            .unwrap();

        let phone_secret = x25519_dalek::StaticSecret::random_from_rng(rand::thread_rng());
        let phone_public = x25519_dalek::PublicKey::from(&phone_secret);

        let request = PairRequest {
            token: start.qr_data.token.clone(),
            phone_pubkey: base64::engine::general_purpose::STANDARD
                .encode(phone_public.as_bytes()),
            device_name: "Test Phone".into(),
            device_model: "TestModel".into(),
        };

        let _rx = mgr.submit_pair_request(&request).unwrap();
        mgr.deny();

        assert!(!mgr.is_active());
        assert!(!mgr.has_pending());
    }

    #[test]
    fn pending_approval_shows_device_info() {
        let mut mgr = PairingManager::new();
        let start = mgr
            .start("https://192.168.1.42:8443".into(), "fp".into())
            .unwrap();

        assert!(mgr.pending_approval().is_none());

        let phone_secret = x25519_dalek::StaticSecret::random_from_rng(rand::thread_rng());
        let phone_public = x25519_dalek::PublicKey::from(&phone_secret);

        let request = PairRequest {
            token: start.qr_data.token.clone(),
            phone_pubkey: base64::engine::general_purpose::STANDARD
                .encode(phone_public.as_bytes()),
            device_name: "Léa's iPhone".into(),
            device_model: "iPhone 15 Pro".into(),
        };

        let _rx = mgr.submit_pair_request(&request).unwrap();

        let pending = mgr.pending_approval().unwrap();
        assert_eq!(pending.device_name, "Léa's iPhone");
        assert_eq!(pending.device_model, "iPhone 15 Pro");
    }

    #[test]
    fn qr_data_contains_all_fields() {
        let mut mgr = PairingManager::new();
        let start = mgr
            .start("https://192.168.1.42:8443".into(), "SHA256:AB:CD".into())
            .unwrap();

        assert_eq!(start.qr_data.v, 1);
        assert_eq!(start.qr_data.url, "https://192.168.1.42:8443");
        assert_eq!(start.qr_data.cert_fp, "SHA256:AB:CD");
        assert!(!start.qr_data.token.is_empty());
        assert!(!start.qr_data.pubkey.is_empty());

        // Verify pubkey is valid base64-encoded 32 bytes
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&start.qr_data.pubkey)
            .unwrap();
        assert_eq!(decoded.len(), 32);
    }

    #[test]
    fn qr_svg_is_valid() {
        let mut mgr = PairingManager::new();
        let start = mgr
            .start("https://192.168.1.42:8443".into(), "fp".into())
            .unwrap();
        assert!(start.qr_svg.contains("<svg"));
        assert!(start.qr_svg.contains("</svg>"));
    }

    // ── Database persistence tests ────────────────────────

    /// Tests the production split-approve flow (RS-M002-01 race fix):
    /// Desktop IPC calls `signal_approval()`, HTTP handler calls `complete_pairing()`.
    #[test]
    fn split_approval_flow_signal_then_complete() {
        let mut mgr = PairingManager::new();
        let start = mgr
            .start("https://192.168.1.42:8443".into(), "fp".into())
            .unwrap();

        let phone_secret = x25519_dalek::StaticSecret::random_from_rng(rand::thread_rng());
        let phone_public = x25519_dalek::PublicKey::from(&phone_secret);

        let request = PairRequest {
            token: start.qr_data.token.clone(),
            phone_pubkey: base64::engine::general_purpose::STANDARD
                .encode(phone_public.as_bytes()),
            device_name: "Split Phone".into(),
            device_model: "SplitModel".into(),
        };

        let _rx = mgr.submit_pair_request(&request).unwrap();

        // Step 1: Desktop signals approval (does NOT consume pairing state)
        mgr.signal_approval().unwrap();

        // State should still be active — pending and active remain
        assert!(mgr.is_active());
        assert!(mgr.has_pending());

        // Step 2: HTTP handler completes pairing (ECDH + token generation)
        let approved = mgr.complete_pairing().unwrap();

        assert_eq!(approved.device_name, "Split Phone");
        assert_eq!(approved.device_model, "SplitModel");
        assert!(!approved.session_token.is_empty());
        assert_ne!(approved.cache_key, [0u8; 32]);

        // State is now consumed
        assert!(!mgr.is_active());
        assert!(!mgr.has_pending());
    }

    /// Calling `signal_approval()` twice should fail (sender already taken).
    #[test]
    fn signal_approval_twice_fails() {
        let mut mgr = PairingManager::new();
        let start = mgr
            .start("https://192.168.1.42:8443".into(), "fp".into())
            .unwrap();

        let phone_secret = x25519_dalek::StaticSecret::random_from_rng(rand::thread_rng());
        let phone_public = x25519_dalek::PublicKey::from(&phone_secret);

        let request = PairRequest {
            token: start.qr_data.token.clone(),
            phone_pubkey: base64::engine::general_purpose::STANDARD
                .encode(phone_public.as_bytes()),
            device_name: "Phone".into(),
            device_model: "Model".into(),
        };

        let _rx = mgr.submit_pair_request(&request).unwrap();

        mgr.signal_approval().unwrap();
        let result = mgr.signal_approval();
        assert!(matches!(result, Err(PairingError::NoPendingApproval)));
    }

    /// Calling `complete_pairing()` without prior `signal_approval()` should still work
    /// (the state is present, just the sender hasn't been used).
    #[test]
    fn complete_pairing_without_signal_works() {
        let mut mgr = PairingManager::new();
        let start = mgr
            .start("https://192.168.1.42:8443".into(), "fp".into())
            .unwrap();

        let phone_secret = x25519_dalek::StaticSecret::random_from_rng(rand::thread_rng());
        let phone_public = x25519_dalek::PublicKey::from(&phone_secret);

        let request = PairRequest {
            token: start.qr_data.token.clone(),
            phone_pubkey: base64::engine::general_purpose::STANDARD
                .encode(phone_public.as_bytes()),
            device_name: "Phone".into(),
            device_model: "Model".into(),
        };

        let _rx = mgr.submit_pair_request(&request).unwrap();

        // Skip signal_approval — complete_pairing should still work
        let approved = mgr.complete_pairing().unwrap();
        assert!(!approved.session_token.is_empty());
    }

    // ── Database persistence tests ────────────────────────

    #[test]
    fn db_store_and_load_paired_device() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let pk: [u8; 32] = rand::random();

        db_store_paired_device(&conn, "dev-1", "Test Phone", "iPhone 15", &pk).unwrap();

        let devices = db_load_paired_devices(&conn).unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device_id, "dev-1");
        assert_eq!(devices[0].device_name, "Test Phone");
        assert_eq!(devices[0].device_model, "iPhone 15");
        assert_eq!(devices[0].public_key, pk);
        assert!(!devices[0].is_revoked);
    }

    #[test]
    fn db_store_session_creates_entry() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let pk: [u8; 32] = rand::random();
        let token_hash: [u8; 32] = rand::random();

        db_store_paired_device(&conn, "dev-1", "Phone", "Model", &pk).unwrap();
        db_store_session(&conn, "dev-1", &token_hash).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM device_sessions WHERE device_id = 'dev-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn db_revoke_removes_sessions() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let pk: [u8; 32] = rand::random();
        let th: [u8; 32] = rand::random();

        db_store_paired_device(&conn, "dev-1", "Phone", "Model", &pk).unwrap();
        db_store_session(&conn, "dev-1", &th).unwrap();

        db_revoke_device(&conn, "dev-1").unwrap();

        let devices = db_load_paired_devices(&conn).unwrap();
        assert!(devices[0].is_revoked);

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM device_sessions WHERE device_id = 'dev-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn db_load_empty_returns_empty_vec() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let devices = db_load_paired_devices(&conn).unwrap();
        assert!(devices.is_empty());
    }

    #[test]
    fn db_multiple_devices() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();

        for i in 0..3 {
            let pk: [u8; 32] = rand::random();
            db_store_paired_device(
                &conn,
                &format!("dev-{i}"),
                &format!("Phone {i}"),
                "Model",
                &pk,
            )
            .unwrap();
        }

        let devices = db_load_paired_devices(&conn).unwrap();
        assert_eq!(devices.len(), 3);
    }

    // ── active_qr_data tests ─────────────────────────────

    #[test]
    fn active_qr_data_returns_some_when_active() {
        let mut mgr = PairingManager::new();
        mgr.start("https://192.168.1.42:8443".into(), "SHA256:AB:CD".into())
            .unwrap();

        let data = mgr.active_qr_data();
        assert!(data.is_some());
        let qr = data.unwrap();
        assert_eq!(qr.v, 1);
        assert_eq!(qr.url, "https://192.168.1.42:8443");
        assert_eq!(qr.cert_fp, "SHA256:AB:CD");
        assert!(!qr.token.is_empty());
        assert!(!qr.pubkey.is_empty());
    }

    #[test]
    fn active_qr_data_returns_none_when_no_session() {
        let mgr = PairingManager::new();
        assert!(mgr.active_qr_data().is_none());
    }

    #[test]
    fn active_qr_data_returns_none_after_cancel() {
        let mut mgr = PairingManager::new();
        mgr.start("https://192.168.1.42:8443".into(), "fp".into())
            .unwrap();
        mgr.cancel();
        assert!(mgr.active_qr_data().is_none());
    }

    #[test]
    fn active_qr_data_returns_none_when_consumed() {
        let mut mgr = PairingManager::new();
        let start = mgr
            .start("https://192.168.1.42:8443".into(), "fp".into())
            .unwrap();

        let phone_secret = x25519_dalek::StaticSecret::random_from_rng(rand::thread_rng());
        let phone_public = x25519_dalek::PublicKey::from(&phone_secret);

        let request = PairRequest {
            token: start.qr_data.token.clone(),
            phone_pubkey: base64::engine::general_purpose::STANDARD
                .encode(phone_public.as_bytes()),
            device_name: "Test Phone".into(),
            device_model: "TestModel".into(),
        };

        let _rx = mgr.submit_pair_request(&request).unwrap();
        // Token is now consumed — should not expose QR data
        assert!(mgr.active_qr_data().is_none());
    }
}
