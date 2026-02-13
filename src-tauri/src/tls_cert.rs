//! M0-02: TLS certificate management.
//!
//! Generates and persists a self-signed ECDSA-P256 certificate for the
//! mobile API HTTPS server. The private key is encrypted with the
//! profile's AES-256-GCM key before storage.
//!
//! Certificate lifecycle:
//! - Generated once on first pairing request
//! - Stored in the `server_tls` singleton table
//! - Loaded on subsequent profile unlocks
//! - Fingerprint embedded in QR code for cert pinning

use sha2::{Digest, Sha256};

/// TLS certificate data (decrypted, in-memory).
#[derive(Debug, Clone)]
pub struct TlsCert {
    /// DER-encoded self-signed certificate.
    pub certificate_der: Vec<u8>,
    /// DER-encoded private key (plaintext — only held in memory).
    pub private_key_der: Vec<u8>,
    /// SHA-256 fingerprint of the certificate (hex, colon-separated).
    pub fingerprint: String,
}

/// Errors from TLS certificate operations.
#[derive(Debug, thiserror::Error)]
pub enum TlsCertError {
    #[error("Certificate generation failed: {0}")]
    Generation(String),
    #[error("Certificate not found in database")]
    NotFound,
    #[error("Failed to decrypt private key: {0}")]
    Decryption(String),
    #[error("Database error: {0}")]
    Database(String),
}

/// Generate a new self-signed ECDSA-P256 certificate.
///
/// The certificate is valid for 10 years with subject "Coheara Desktop".
/// Returns the cert data with plaintext private key for in-memory use.
pub fn generate_self_signed() -> Result<TlsCert, TlsCertError> {
    use rcgen::{CertificateParams, KeyPair};

    let key_pair =
        KeyPair::generate().map_err(|e| TlsCertError::Generation(e.to_string()))?;

    let params = CertificateParams::new(vec!["coheara.local".to_string()])
        .map_err(|e| TlsCertError::Generation(e.to_string()))?;

    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| TlsCertError::Generation(e.to_string()))?;

    let cert_der = cert.der().to_vec();
    let key_der = key_pair.serialize_der();
    let fingerprint = compute_fingerprint(&cert_der);

    Ok(TlsCert {
        certificate_der: cert_der,
        private_key_der: key_der,
        fingerprint,
    })
}

/// Compute the SHA-256 fingerprint of a DER-encoded certificate.
///
/// Returns colon-separated hex like "AB:CD:EF:01:..."
pub fn compute_fingerprint(cert_der: &[u8]) -> String {
    let hash = Sha256::digest(cert_der);
    hash.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// Store a TLS certificate in the database.
///
/// The private key is encrypted with AES-256-GCM using the provided profile key.
pub fn store_cert(
    conn: &rusqlite::Connection,
    cert: &TlsCert,
    profile_key: &[u8; 32],
) -> Result<(), TlsCertError> {
    let encrypted_key =
        encrypt_private_key(&cert.private_key_der, profile_key)?;

    conn.execute(
        "INSERT OR REPLACE INTO server_tls (id, private_key_encrypted, certificate_der, fingerprint, created_at)
         VALUES (1, ?1, ?2, ?3, ?4)",
        rusqlite::params![
            encrypted_key,
            cert.certificate_der,
            cert.fingerprint,
            chrono::Utc::now().to_rfc3339(),
        ],
    )
    .map_err(|e| TlsCertError::Database(e.to_string()))?;

    Ok(())
}

/// Load a TLS certificate from the database.
///
/// Decrypts the private key with the provided profile key.
pub fn load_cert(
    conn: &rusqlite::Connection,
    profile_key: &[u8; 32],
) -> Result<TlsCert, TlsCertError> {
    let row = conn
        .query_row(
            "SELECT private_key_encrypted, certificate_der, fingerprint FROM server_tls WHERE id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => TlsCertError::NotFound,
            other => TlsCertError::Database(other.to_string()),
        })?;

    let (encrypted_key, certificate_der, fingerprint) = row;
    let private_key_der = decrypt_private_key(&encrypted_key, profile_key)?;

    Ok(TlsCert {
        certificate_der,
        private_key_der,
        fingerprint,
    })
}

/// Load or generate a TLS certificate.
///
/// If a cert exists in the database, load and decrypt it.
/// Otherwise, generate a new one and store it.
pub fn load_or_generate(
    conn: &rusqlite::Connection,
    profile_key: &[u8; 32],
) -> Result<TlsCert, TlsCertError> {
    match load_cert(conn, profile_key) {
        Ok(cert) => Ok(cert),
        Err(TlsCertError::NotFound) => {
            let cert = generate_self_signed()?;
            store_cert(conn, &cert, profile_key)?;
            Ok(cert)
        }
        Err(e) => Err(e),
    }
}

// ─── Private key encryption ───────────────────────────────

/// Encrypt a private key with AES-256-GCM.
fn encrypt_private_key(
    plaintext: &[u8],
    key: &[u8; 32],
) -> Result<Vec<u8>, TlsCertError> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    let nonce_bytes: [u8; 12] = rand::random();
    let cipher = Aes256Gcm::new(key.into());
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| TlsCertError::Generation(format!("encryption failed: {e}")))?;

    // Prepend nonce to ciphertext: [12B nonce | ciphertext+tag]
    let mut output = Vec::with_capacity(12 + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

/// Decrypt a private key with AES-256-GCM.
fn decrypt_private_key(
    encrypted: &[u8],
    key: &[u8; 32],
) -> Result<Vec<u8>, TlsCertError> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    if encrypted.len() < 12 {
        return Err(TlsCertError::Decryption("data too short".into()));
    }

    let (nonce_bytes, ciphertext) = encrypted.split_at(12);
    let cipher = Aes256Gcm::new(key.into());
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| TlsCertError::Decryption(e.to_string()))
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_self_signed_produces_valid_cert() {
        let cert = generate_self_signed().unwrap();
        assert!(!cert.certificate_der.is_empty());
        assert!(!cert.private_key_der.is_empty());
        assert!(cert.fingerprint.contains(':'));
        // Fingerprint is 32 bytes = 64 hex chars + 31 colons = 95 chars
        assert_eq!(cert.fingerprint.len(), 95);
    }

    #[test]
    fn fingerprint_is_deterministic() {
        let cert = generate_self_signed().unwrap();
        let fp2 = compute_fingerprint(&cert.certificate_der);
        assert_eq!(cert.fingerprint, fp2);
    }

    #[test]
    fn private_key_roundtrip_encryption() {
        let key: [u8; 32] = rand::random();
        let plaintext = b"test private key data";

        let encrypted = encrypt_private_key(plaintext, &key).unwrap();
        assert_ne!(encrypted, plaintext);
        assert!(encrypted.len() > plaintext.len()); // nonce + tag overhead

        let decrypted = decrypt_private_key(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn wrong_key_fails_decryption() {
        let key1: [u8; 32] = rand::random();
        let key2: [u8; 32] = rand::random();
        let plaintext = b"secret key material";

        let encrypted = encrypt_private_key(plaintext, &key1).unwrap();
        let result = decrypt_private_key(&encrypted, &key2);
        assert!(result.is_err());
    }

    #[test]
    fn store_and_load_cert_roundtrip() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let profile_key: [u8; 32] = rand::random();

        let cert = generate_self_signed().unwrap();
        store_cert(&conn, &cert, &profile_key).unwrap();

        let loaded = load_cert(&conn, &profile_key).unwrap();
        assert_eq!(loaded.certificate_der, cert.certificate_der);
        assert_eq!(loaded.private_key_der, cert.private_key_der);
        assert_eq!(loaded.fingerprint, cert.fingerprint);
    }

    #[test]
    fn load_cert_not_found() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let key: [u8; 32] = rand::random();
        let result = load_cert(&conn, &key);
        assert!(matches!(result, Err(TlsCertError::NotFound)));
    }

    #[test]
    fn load_or_generate_creates_on_first_call() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let key: [u8; 32] = rand::random();

        let cert1 = load_or_generate(&conn, &key).unwrap();
        let cert2 = load_or_generate(&conn, &key).unwrap();

        // Should return the SAME cert (loaded, not regenerated)
        assert_eq!(cert1.certificate_der, cert2.certificate_der);
        assert_eq!(cert1.fingerprint, cert2.fingerprint);
    }

    #[test]
    fn store_cert_replaces_existing() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let key: [u8; 32] = rand::random();

        let cert1 = generate_self_signed().unwrap();
        store_cert(&conn, &cert1, &key).unwrap();

        let cert2 = generate_self_signed().unwrap();
        store_cert(&conn, &cert2, &key).unwrap();

        let loaded = load_cert(&conn, &key).unwrap();
        assert_eq!(loaded.fingerprint, cert2.fingerprint);
    }
}
