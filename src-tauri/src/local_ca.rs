//! SEC-HTTPS-01: Local Certificate Authority for HTTPS on local networks.
//!
//! Generates a local CA + server certificates that iOS Safari trusts
//! once the user installs the CA profile. Pattern: Home Assistant + Synology.
//!
//! Architecture:
//! - CA cert: long-lived (825 days max per Apple), persisted in profile DB
//! - Server cert: session-scoped, issued per IP/server start
//! - .mobileconfig: iOS configuration profile for one-tap CA install
//!
//! Apple TLS requirements (iOS 13+, support.apple.com/103769):
//! - SHA-2 hash algorithm (ECDSA P-256 with SHA-256)
//! - SAN with DNS/IP (CN alone NOT trusted)
//! - ExtendedKeyUsage: id-kp-serverAuth
//! - Validity <= 825 days (CA), <= 398 days (server cert)
//! - Authority Key Identifier on server certs
//!
//! Industry patterns: Home Assistant (local CA + guided trust),
//! Synology (.mobileconfig download), Plex (DNS rebinding — requires internet).

use std::net::IpAddr;

use base64::Engine;
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa,
    KeyPair, KeyUsagePurpose, PKCS_ECDSA_P256_SHA256,
};
use rustls_pki_types::PrivatePkcs8KeyDer;
use time::{Duration, OffsetDateTime};

use crate::tls_cert::compute_fingerprint;

// ═══════════════════════════════════════════════════════════
// Public types
// ═══════════════════════════════════════════════════════════

/// CA bundle: everything needed to persist and reuse a local CA.
///
/// The `cert_der` is the original CA certificate served to phones.
/// The `key_der` is the CA private key (encrypt before storing).
#[derive(Debug, Clone)]
pub struct CaBundle {
    /// DER-encoded CA certificate (served to phones, never regenerated).
    pub cert_der: Vec<u8>,
    /// DER-encoded CA private key (PKCS#8 — encrypt at rest with profile key).
    pub key_der: Vec<u8>,
    /// SHA-256 fingerprint of the CA cert (colon-separated hex).
    pub fingerprint: String,
}

/// Server certificate bundle: cert + key signed by the local CA.
///
/// Contains everything needed to configure a rustls HTTPS server
/// and serve the CA certificate to phones for trust installation.
#[derive(Debug, Clone)]
pub struct ServerCertBundle {
    /// DER-encoded server certificate.
    pub cert_der: Vec<u8>,
    /// DER-encoded server private key (PKCS#8).
    pub key_der: Vec<u8>,
    /// SHA-256 fingerprint of the server cert (for QR code cert pinning).
    pub fingerprint: String,
    /// DER-encoded CA certificate (for chain: server cert → CA cert).
    pub ca_cert_der: Vec<u8>,
}

/// Errors from Local CA operations.
#[derive(Debug, thiserror::Error)]
pub enum LocalCaError {
    #[error("CA generation failed: {0}")]
    CaGeneration(String),
    #[error("Server certificate generation failed: {0}")]
    ServerCert(String),
    #[error("CA key reconstruction failed: {0}")]
    KeyReconstruction(String),
    #[error("CA not found in database")]
    NotFound,
    #[error("Failed to decrypt CA key: {0}")]
    Decryption(String),
    #[error("Database error: {0}")]
    Database(String),
}

// ═══════════════════════════════════════════════════════════
// Certificate generation — pure functions, no I/O
// ═══════════════════════════════════════════════════════════

/// Apple's maximum validity for CA certificates (days).
const CA_VALIDITY_DAYS: i64 = 825;

/// Apple's maximum validity for server certificates (days).
const SERVER_CERT_VALIDITY_DAYS: i64 = 398;

/// Clock skew grace period (hours).
const CLOCK_SKEW_HOURS: i64 = 1;

/// Distinguished Name: Common Name for the CA.
const CA_CN: &str = "Coheara Local CA";

/// Distinguished Name: Organization for all certs.
const CA_ORG: &str = "Coheara";

/// Generate a new local Certificate Authority.
///
/// The CA uses ECDSA P-256 (compact, fast, Apple-compliant) and is valid
/// for 825 days (Apple maximum). Returns a `CaBundle` with DER-encoded
/// cert + key for persistence.
pub fn generate_ca() -> Result<CaBundle, LocalCaError> {
    let mut params = CertificateParams::new(Vec::default())
        .map_err(|e| LocalCaError::CaGeneration(e.to_string()))?;

    // CA identity
    params.distinguished_name.push(DnType::CommonName, CA_CN);
    params
        .distinguished_name
        .push(DnType::OrganizationName, CA_ORG);

    // CA constraints (required for signing server certs)
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages.push(KeyUsagePurpose::DigitalSignature);
    params.key_usages.push(KeyUsagePurpose::KeyCertSign);
    params.key_usages.push(KeyUsagePurpose::CrlSign);

    // Validity: Apple requires <= 825 days
    let now = OffsetDateTime::now_utc();
    params.not_before = now - Duration::hours(CLOCK_SKEW_HOURS);
    params.not_after = now + Duration::days(CA_VALIDITY_DAYS);

    let key =
        KeyPair::generate().map_err(|e| LocalCaError::CaGeneration(e.to_string()))?;
    let cert = params
        .self_signed(&key)
        .map_err(|e| LocalCaError::CaGeneration(e.to_string()))?;

    let cert_der = cert.der().to_vec();
    let fingerprint = compute_fingerprint(&cert_der);

    Ok(CaBundle {
        cert_der,
        key_der: key.serialize_der(),
        fingerprint,
    })
}

/// Issue a server certificate signed by the local CA.
///
/// The server cert includes the local IP and hostnames in the SAN
/// extension, meeting Apple's iOS 13+ requirements. Valid for 398 days.
///
/// The `ca` bundle must contain a valid CA key (decrypted).
pub fn issue_server_cert(
    ca: &CaBundle,
    local_ip: IpAddr,
) -> Result<ServerCertBundle, LocalCaError> {
    // Reconstruct CA signing identity from stored key
    let pkcs8 = PrivatePkcs8KeyDer::from(ca.key_der.as_slice());
    let ca_key = KeyPair::from_pkcs8_der_and_sign_algo(&pkcs8, &PKCS_ECDSA_P256_SHA256)
        .map_err(|e| LocalCaError::KeyReconstruction(e.to_string()))?;
    let ca_cert = reconstruct_signing_ca(&ca_key)?;

    // Server cert SANs: IP + hostnames (Apple requires SAN, not just CN)
    let san_names = vec![
        local_ip.to_string(),
        "127.0.0.1".to_string(),
        "coheara.local".to_string(),
        "localhost".to_string(),
    ];

    let mut params = CertificateParams::new(san_names)
        .map_err(|e| LocalCaError::ServerCert(e.to_string()))?;

    // Server identity
    params
        .distinguished_name
        .push(DnType::CommonName, "Coheara Server");
    params
        .distinguished_name
        .push(DnType::OrganizationName, CA_ORG);

    // EKU: serverAuth (required by Apple iOS 13+)
    params
        .extended_key_usages
        .push(ExtendedKeyUsagePurpose::ServerAuth);
    params.key_usages.push(KeyUsagePurpose::DigitalSignature);
    params.key_usages.push(KeyUsagePurpose::KeyEncipherment);

    // Authority Key Identifier links server cert to CA
    params.use_authority_key_identifier_extension = true;

    // Validity: Apple recommends <= 398 days for server certs
    let now = OffsetDateTime::now_utc();
    params.not_before = now - Duration::hours(CLOCK_SKEW_HOURS);
    params.not_after = now + Duration::days(SERVER_CERT_VALIDITY_DAYS);

    let server_key =
        KeyPair::generate().map_err(|e| LocalCaError::ServerCert(e.to_string()))?;
    let server_cert = params
        .signed_by(&server_key, &ca_cert, &ca_key)
        .map_err(|e| LocalCaError::ServerCert(e.to_string()))?;

    let cert_der = server_cert.der().to_vec();
    let fingerprint = compute_fingerprint(&cert_der);

    Ok(ServerCertBundle {
        cert_der,
        key_der: server_key.serialize_der(),
        fingerprint,
        ca_cert_der: ca.cert_der.clone(),
    })
}

// ═══════════════════════════════════════════════════════════
// Export formats — .mobileconfig + PEM
// ═══════════════════════════════════════════════════════════

/// Build an iOS `.mobileconfig` profile for the CA certificate.
///
/// When served with `Content-Type: application/x-apple-aspen-config`,
/// Safari prompts the user to install the profile. After installation,
/// the user must also enable trust in Settings > General > About >
/// Certificate Trust Settings.
///
/// Pattern: Home Assistant, Synology, mkcert.
pub fn build_mobileconfig(ca_cert_der: &[u8]) -> String {
    let ca_b64 = base64::engine::general_purpose::STANDARD.encode(ca_cert_der);
    let payload_uuid = uuid::Uuid::new_v4();
    let profile_uuid = uuid::Uuid::new_v4();

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>PayloadContent</key>
    <array>
        <dict>
            <key>PayloadCertificateFileName</key>
            <string>CohearaCA.cer</string>
            <key>PayloadContent</key>
            <data>{ca_b64}</data>
            <key>PayloadDescription</key>
            <string>Adds the Coheara local CA for secure connections to your desktop app</string>
            <key>PayloadDisplayName</key>
            <string>Coheara Local CA</string>
            <key>PayloadOrganization</key>
            <string>Coheara</string>
            <key>PayloadIdentifier</key>
            <string>com.coheara.local-ca</string>
            <key>PayloadType</key>
            <string>com.apple.security.root</string>
            <key>PayloadUUID</key>
            <string>{payload_uuid}</string>
            <key>PayloadVersion</key>
            <integer>1</integer>
        </dict>
    </array>
    <key>PayloadDisplayName</key>
    <string>Coheara Companion Setup</string>
    <key>PayloadOrganization</key>
    <string>Coheara</string>
    <key>PayloadIdentifier</key>
    <string>com.coheara.companion-profile</string>
    <key>PayloadRemovalDisallowed</key>
    <false/>
    <key>PayloadType</key>
    <string>Configuration</string>
    <key>PayloadUUID</key>
    <string>{profile_uuid}</string>
    <key>PayloadVersion</key>
    <integer>1</integer>
    <key>PayloadDescription</key>
    <string>Enables secure HTTPS connection between your phone and Coheara desktop</string>
</dict>
</plist>"#,
        ca_b64 = ca_b64,
        payload_uuid = payload_uuid,
        profile_uuid = profile_uuid,
    )
}

/// Export CA certificate as PEM (for Android and generic clients).
///
/// Android: Settings > Security > Install CA certificate.
pub fn export_ca_pem(ca_cert_der: &[u8]) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(ca_cert_der);
    // PEM wraps base64 at 64 chars per line
    let wrapped: Vec<&str> = b64
        .as_bytes()
        .chunks(64)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
        .collect();
    format!(
        "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
        wrapped.join("\n")
    )
}

// ═══════════════════════════════════════════════════════════
// Persistence — encrypted CA storage in profile DB
// ═══════════════════════════════════════════════════════════

/// Store a CA bundle in the profile database.
///
/// The CA private key is encrypted with AES-256-GCM using the profile key.
/// The CA certificate (public) is stored unencrypted — it must be served
/// to phones for trust installation.
pub fn store_ca(
    conn: &rusqlite::Connection,
    ca: &CaBundle,
    profile_key: &[u8; 32],
) -> Result<(), LocalCaError> {
    let encrypted_key = encrypt_key(&ca.key_der, profile_key)?;

    conn.execute(
        "INSERT OR REPLACE INTO local_ca (id, cert_der, key_encrypted, fingerprint, created_at)
         VALUES (1, ?1, ?2, ?3, ?4)",
        rusqlite::params![
            ca.cert_der,
            encrypted_key,
            ca.fingerprint,
            chrono::Utc::now().to_rfc3339(),
        ],
    )
    .map_err(|e| LocalCaError::Database(e.to_string()))?;

    Ok(())
}

/// Load a CA bundle from the profile database.
///
/// Decrypts the CA private key with the profile key.
pub fn load_ca(
    conn: &rusqlite::Connection,
    profile_key: &[u8; 32],
) -> Result<CaBundle, LocalCaError> {
    let row = conn
        .query_row(
            "SELECT cert_der, key_encrypted, fingerprint FROM local_ca WHERE id = 1",
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
            rusqlite::Error::QueryReturnedNoRows => LocalCaError::NotFound,
            other => LocalCaError::Database(other.to_string()),
        })?;

    let (cert_der, encrypted_key, fingerprint) = row;
    let key_der = decrypt_key(&encrypted_key, profile_key)?;

    Ok(CaBundle {
        cert_der,
        key_der,
        fingerprint,
    })
}

/// Load an existing CA or generate a new one.
///
/// On first call: generates CA, stores encrypted, returns bundle.
/// On subsequent calls: loads and decrypts the existing CA.
pub fn load_or_generate_ca(
    conn: &rusqlite::Connection,
    profile_key: &[u8; 32],
) -> Result<CaBundle, LocalCaError> {
    match load_ca(conn, profile_key) {
        Ok(ca) => Ok(ca),
        Err(LocalCaError::NotFound) => {
            let ca = generate_ca()?;
            store_ca(conn, &ca, profile_key)?;
            Ok(ca)
        }
        Err(e) => Err(e),
    }
}

// ═══════════════════════════════════════════════════════════
// Internal — CA reconstruction + crypto helpers
// ═══════════════════════════════════════════════════════════

/// Reconstruct a CA Certificate from its key pair for signing.
///
/// The reconstructed cert has the same DN and key as the original,
/// producing valid cert chains (iOS validates by public key, not
/// exact cert bytes). The original cert DER (stored separately)
/// is what phones trust.
fn reconstruct_signing_ca(key: &KeyPair) -> Result<Certificate, LocalCaError> {
    let mut params = CertificateParams::new(Vec::default())
        .map_err(|e| LocalCaError::CaGeneration(e.to_string()))?;

    params.distinguished_name.push(DnType::CommonName, CA_CN);
    params
        .distinguished_name
        .push(DnType::OrganizationName, CA_ORG);
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages.push(KeyUsagePurpose::DigitalSignature);
    params.key_usages.push(KeyUsagePurpose::KeyCertSign);
    params.key_usages.push(KeyUsagePurpose::CrlSign);

    let now = OffsetDateTime::now_utc();
    params.not_before = now - Duration::hours(CLOCK_SKEW_HOURS);
    params.not_after = now + Duration::days(CA_VALIDITY_DAYS);

    params
        .self_signed(key)
        .map_err(|e| LocalCaError::CaGeneration(e.to_string()))
}

/// Encrypt a private key with AES-256-GCM (reuses tls_cert pattern).
fn encrypt_key(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, LocalCaError> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    let nonce_bytes: [u8; 12] = rand::random();
    let cipher = Aes256Gcm::new(key.into());
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| LocalCaError::CaGeneration(format!("encryption failed: {e}")))?;

    let mut output = Vec::with_capacity(12 + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

/// Decrypt a private key with AES-256-GCM.
fn decrypt_key(encrypted: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, LocalCaError> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    if encrypted.len() < 12 {
        return Err(LocalCaError::Decryption("data too short".into()));
    }

    let (nonce_bytes, ciphertext) = encrypted.split_at(12);
    let cipher = Aes256Gcm::new(key.into());
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| LocalCaError::Decryption(e.to_string()))
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    // ── CA generation ────────────────────────────────────

    #[test]
    fn generate_ca_produces_valid_bundle() {
        let ca = generate_ca().unwrap();
        assert!(!ca.cert_der.is_empty());
        assert!(!ca.key_der.is_empty());
        assert!(ca.fingerprint.contains(':'));
        // SHA-256 fingerprint: 32 bytes = 64 hex + 31 colons = 95 chars
        assert_eq!(ca.fingerprint.len(), 95);
    }

    #[test]
    fn generate_ca_produces_unique_certs() {
        let ca1 = generate_ca().unwrap();
        let ca2 = generate_ca().unwrap();
        assert_ne!(ca1.cert_der, ca2.cert_der);
        assert_ne!(ca1.key_der, ca2.key_der);
        assert_ne!(ca1.fingerprint, ca2.fingerprint);
    }

    #[test]
    fn ca_key_roundtrip_der() {
        let ca = generate_ca().unwrap();
        let pkcs8 = PrivatePkcs8KeyDer::from(ca.key_der.as_slice());
        let key_restored = KeyPair::from_pkcs8_der_and_sign_algo(&pkcs8, &PKCS_ECDSA_P256_SHA256).unwrap();
        let pkcs8_2 = PrivatePkcs8KeyDer::from(ca.key_der.as_slice());
        let key_again = KeyPair::from_pkcs8_der_and_sign_algo(&pkcs8_2, &PKCS_ECDSA_P256_SHA256).unwrap();
        // Same public key bytes
        assert_eq!(key_restored.public_key_der(), key_again.public_key_der());
    }

    // ── Server cert issuance ─────────────────────────────

    #[test]
    fn issue_server_cert_ipv4() {
        let ca = generate_ca().unwrap();
        let ip: IpAddr = "192.168.1.100".parse().unwrap();

        let server = issue_server_cert(&ca, ip).unwrap();
        assert!(!server.cert_der.is_empty());
        assert!(!server.key_der.is_empty());
        assert!(server.fingerprint.contains(':'));
        assert_eq!(server.fingerprint.len(), 95);
        assert_eq!(server.ca_cert_der, ca.cert_der);
    }

    #[test]
    fn issue_server_cert_ipv6() {
        let ca = generate_ca().unwrap();
        let ip: IpAddr = "::1".parse().unwrap();

        let server = issue_server_cert(&ca, ip).unwrap();
        assert!(!server.cert_der.is_empty());
    }

    #[test]
    fn server_cert_differs_from_ca() {
        let ca = generate_ca().unwrap();
        let ip: IpAddr = "10.0.0.1".parse().unwrap();

        let server = issue_server_cert(&ca, ip).unwrap();
        assert_ne!(server.cert_der, ca.cert_der);
        assert_ne!(server.key_der, ca.key_der);
        assert_ne!(server.fingerprint, ca.fingerprint);
    }

    #[test]
    fn multiple_server_certs_from_same_ca() {
        let ca = generate_ca().unwrap();
        let ip1: IpAddr = "192.168.1.1".parse().unwrap();
        let ip2: IpAddr = "10.0.0.1".parse().unwrap();

        let s1 = issue_server_cert(&ca, ip1).unwrap();
        let s2 = issue_server_cert(&ca, ip2).unwrap();

        // Different certs from same CA
        assert_ne!(s1.cert_der, s2.cert_der);
        // Same CA cert
        assert_eq!(s1.ca_cert_der, s2.ca_cert_der);
    }

    // ── iOS compliance ───────────────────────────────────

    #[test]
    fn server_cert_fingerprint_matches_recompute() {
        let ca = generate_ca().unwrap();
        let ip: IpAddr = "192.168.1.50".parse().unwrap();

        let server = issue_server_cert(&ca, ip).unwrap();
        let recomputed = compute_fingerprint(&server.cert_der);
        assert_eq!(server.fingerprint, recomputed);
    }

    // ── .mobileconfig export ─────────────────────────────

    #[test]
    fn mobileconfig_is_valid_xml() {
        let ca = generate_ca().unwrap();
        let config = build_mobileconfig(&ca.cert_der);

        assert!(config.starts_with("<?xml"));
        assert!(config.contains("PayloadType"));
        assert!(config.contains("com.apple.security.root"));
        assert!(config.contains("Coheara Local CA"));
        assert!(config.contains("Coheara Companion Setup"));
        assert!(config.contains("PayloadOrganization"));
        assert!(config.contains("<data>"));
        assert!(config.contains("</plist>"));
    }

    #[test]
    fn mobileconfig_contains_cert_data() {
        let ca = generate_ca().unwrap();
        let config = build_mobileconfig(&ca.cert_der);

        // The base64-encoded CA cert should appear in the profile
        let ca_b64 = base64::engine::general_purpose::STANDARD.encode(&ca.cert_der);
        assert!(config.contains(&ca_b64));
    }

    #[test]
    fn mobileconfig_has_unique_uuids() {
        let ca = generate_ca().unwrap();
        let config1 = build_mobileconfig(&ca.cert_der);
        let config2 = build_mobileconfig(&ca.cert_der);

        // Each call generates fresh UUIDs
        assert_ne!(config1, config2);
    }

    // ── PEM export ───────────────────────────────────────

    #[test]
    fn ca_pem_has_correct_format() {
        let ca = generate_ca().unwrap();
        let pem = export_ca_pem(&ca.cert_der);

        assert!(pem.starts_with("-----BEGIN CERTIFICATE-----\n"));
        assert!(pem.ends_with("-----END CERTIFICATE-----\n"));
        // Lines should be <= 64 chars (PEM standard)
        for line in pem.lines() {
            if line.starts_with("-----") {
                continue;
            }
            assert!(
                line.len() <= 64,
                "PEM line exceeds 64 chars: {} ({})",
                line.len(),
                line
            );
        }
    }

    #[test]
    fn ca_pem_roundtrip() {
        let ca = generate_ca().unwrap();
        let pem = export_ca_pem(&ca.cert_der);

        // Extract base64 from PEM and decode back to DER
        let b64: String = pem
            .lines()
            .filter(|l| !l.starts_with("-----"))
            .collect::<Vec<_>>()
            .join("");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&b64)
            .unwrap();
        assert_eq!(decoded, ca.cert_der);
    }

    // ── Persistence ──────────────────────────────────────

    #[test]
    fn store_and_load_ca_roundtrip() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let profile_key: [u8; 32] = rand::random();

        let ca = generate_ca().unwrap();
        store_ca(&conn, &ca, &profile_key).unwrap();

        let loaded = load_ca(&conn, &profile_key).unwrap();
        assert_eq!(loaded.cert_der, ca.cert_der);
        assert_eq!(loaded.key_der, ca.key_der);
        assert_eq!(loaded.fingerprint, ca.fingerprint);
    }

    #[test]
    fn load_ca_not_found() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let key: [u8; 32] = rand::random();
        let result = load_ca(&conn, &key);
        assert!(matches!(result, Err(LocalCaError::NotFound)));
    }

    #[test]
    fn load_or_generate_creates_on_first_call() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let key: [u8; 32] = rand::random();

        let ca1 = load_or_generate_ca(&conn, &key).unwrap();
        let ca2 = load_or_generate_ca(&conn, &key).unwrap();

        // Same CA (loaded, not regenerated)
        assert_eq!(ca1.cert_der, ca2.cert_der);
        assert_eq!(ca1.fingerprint, ca2.fingerprint);
    }

    #[test]
    fn wrong_key_fails_decryption() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let key1: [u8; 32] = rand::random();
        let key2: [u8; 32] = rand::random();

        let ca = generate_ca().unwrap();
        store_ca(&conn, &ca, &key1).unwrap();

        let result = load_ca(&conn, &key2);
        assert!(matches!(result, Err(LocalCaError::Decryption(_))));
    }

    #[test]
    fn stored_ca_can_issue_server_certs() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let profile_key: [u8; 32] = rand::random();

        let ca = load_or_generate_ca(&conn, &profile_key).unwrap();
        let ip: IpAddr = "192.168.1.42".parse().unwrap();

        let server = issue_server_cert(&ca, ip).unwrap();
        assert!(!server.cert_der.is_empty());
        assert_eq!(server.ca_cert_der, ca.cert_der);
    }

    // ── Key encryption ───────────────────────────────────

    #[test]
    fn key_encryption_roundtrip() {
        let key: [u8; 32] = rand::random();
        let plaintext = b"test CA private key material";

        let encrypted = encrypt_key(plaintext, &key).unwrap();
        assert_ne!(encrypted.as_slice(), plaintext);

        let decrypted = decrypt_key(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn truncated_encrypted_data_fails() {
        let key: [u8; 32] = rand::random();
        let result = decrypt_key(&[0u8; 5], &key);
        assert!(matches!(result, Err(LocalCaError::Decryption(_))));
    }

    #[test]
    fn store_ca_replaces_existing() {
        let conn = crate::db::sqlite::open_memory_database().unwrap();
        let key: [u8; 32] = rand::random();

        let ca1 = generate_ca().unwrap();
        store_ca(&conn, &ca1, &key).unwrap();

        let ca2 = generate_ca().unwrap();
        store_ca(&conn, &ca2, &key).unwrap();

        let loaded = load_ca(&conn, &key).unwrap();
        assert_eq!(loaded.fingerprint, ca2.fingerprint);
    }
}
