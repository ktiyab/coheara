use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use serde::{Deserialize, Serialize};

use super::CryptoError;
use super::keys::KEY_LENGTH;

const NONCE_LENGTH: usize = 12;
/// Magic byte marking versioned EncryptedData format.
/// Chosen as 0xCE ("Coheara Encrypted") to distinguish from legacy unversioned data.
/// Legacy nonces are random bytes — P(byte[0]==0xCE AND byte[1]==known_version) ≈ 1/65536.
const ENCRYPTED_MAGIC: u8 = 0xCE;
/// Current encryption format version (AES-256-GCM with 12-byte nonce).
/// Version history: 0x01 = initial versioned format.
const CURRENT_VERSION: u8 = 0x01;

/// Encrypted data container: nonce + ciphertext (includes AES-GCM auth tag)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    pub nonce: [u8; NONCE_LENGTH],
    pub ciphertext: Vec<u8>,
}

impl EncryptedData {
    /// Encrypt plaintext using AES-256-GCM with a random nonce
    pub(crate) fn encrypt(key_bytes: &[u8; KEY_LENGTH], plaintext: &[u8]) -> Result<Self, CryptoError> {
        let key = Key::<Aes256Gcm>::from_slice(key_bytes);
        let cipher = Aes256Gcm::new(key);

        let mut nonce_bytes = [0u8; NONCE_LENGTH];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| CryptoError::EncryptionFailed)?;

        Ok(Self {
            nonce: nonce_bytes,
            ciphertext,
        })
    }

    /// Decrypt ciphertext using AES-256-GCM
    pub(crate) fn decrypt(&self, key_bytes: &[u8; KEY_LENGTH]) -> Result<Vec<u8>, CryptoError> {
        let key = Key::<Aes256Gcm>::from_slice(key_bytes);
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(&self.nonce);

        cipher
            .decrypt(nonce, self.ciphertext.as_ref())
            .map_err(|_| CryptoError::DecryptionFailed)
    }

    /// Serialize to bytes: `[0xCE][version][12-byte nonce][ciphertext...]`
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(2 + NONCE_LENGTH + self.ciphertext.len());
        bytes.push(ENCRYPTED_MAGIC);
        bytes.push(CURRENT_VERSION);
        bytes.extend_from_slice(&self.nonce);
        bytes.extend_from_slice(&self.ciphertext);
        bytes
    }

    /// Deserialize from bytes. Supports both formats:
    /// - **Versioned**: `[0xCE][version][12-byte nonce][ciphertext...]`
    /// - **Legacy**: `[12-byte nonce][ciphertext...]` (no magic/version prefix)
    ///
    /// Detection: if first byte is `0xCE` (magic marker), parse as versioned.
    /// Otherwise treat entire input as legacy format. Collision probability with
    /// random legacy nonces: P(byte[0]==0xCE ∧ byte[1]==known_version) ≈ 1/65536.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.is_empty() {
            return Err(CryptoError::CorruptedProfile);
        }

        let nonce_start = if bytes[0] == ENCRYPTED_MAGIC {
            // Versioned format — check version byte
            if bytes.len() < 2 {
                return Err(CryptoError::CorruptedProfile);
            }
            match bytes[1] {
                CURRENT_VERSION => 2, // skip magic + version
                _ => return Err(CryptoError::CorruptedProfile),
            }
        } else {
            // Legacy unversioned format — nonce starts at byte 0
            0
        };

        let min_len = nonce_start + NONCE_LENGTH + 16; // nonce + AES-GCM tag minimum
        if bytes.len() < min_len {
            return Err(CryptoError::CorruptedProfile);
        }

        let mut nonce = [0u8; NONCE_LENGTH];
        nonce.copy_from_slice(&bytes[nonce_start..nonce_start + NONCE_LENGTH]);
        let ciphertext = bytes[nonce_start + NONCE_LENGTH..].to_vec();

        Ok(Self { nonce, ciphertext })
    }
}

/// Encrypt a file and write to disk
pub fn encrypt_file(
    key: &super::ProfileKey,
    plaintext_path: &std::path::Path,
    encrypted_path: &std::path::Path,
) -> Result<(), CryptoError> {
    let plaintext = std::fs::read(plaintext_path)?;
    let encrypted = key.encrypt(&plaintext)?;
    std::fs::write(encrypted_path, encrypted.to_bytes())?;
    Ok(())
}

/// Decrypt a file from disk
pub fn decrypt_file(
    key: &super::ProfileKey,
    encrypted_path: &std::path::Path,
) -> Result<Vec<u8>, CryptoError> {
    let bytes = std::fs::read(encrypted_path)?;
    let encrypted = EncryptedData::from_bytes(&bytes)?;
    key.decrypt(&encrypted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::keys::ProfileKey;

    fn test_key() -> ProfileKey {
        // Use minimal iterations for test speed — tests for timing are in keys.rs
        ProfileKey::derive("test_password", &[0u8; 32])
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let key = test_key();
        let plaintext = b"Hello, Coheara medical data!";
        let encrypted = key.encrypt(plaintext).unwrap();
        let decrypted = key.decrypt(&encrypted).unwrap();
        assert_eq!(&decrypted, plaintext);
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let key1 = ProfileKey::derive("password1", &[0u8; 32]);
        let key2 = ProfileKey::derive("password2", &[0u8; 32]);
        let encrypted = key1.encrypt(b"secret").unwrap();
        let result = key2.decrypt(&encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn tampered_ciphertext_detected() {
        let key = test_key();
        let encrypted = key.encrypt(b"secret data").unwrap();
        let mut tampered = encrypted.clone();
        tampered.ciphertext[0] ^= 0xFF;
        assert!(key.decrypt(&tampered).is_err());
    }

    #[test]
    fn encrypted_data_serialization_round_trip() {
        let key = test_key();
        let encrypted = key.encrypt(b"serialize me").unwrap();
        let bytes = encrypted.to_bytes();
        let restored = EncryptedData::from_bytes(&bytes).unwrap();
        let decrypted = key.decrypt(&restored).unwrap();
        assert_eq!(&decrypted, b"serialize me");
    }

    #[test]
    fn from_bytes_rejects_too_short() {
        let result = EncryptedData::from_bytes(&[0u8; 10]);
        assert!(result.is_err());
    }

    #[test]
    fn different_encryptions_produce_different_nonces() {
        let key = test_key();
        let e1 = key.encrypt(b"same data").unwrap();
        let e2 = key.encrypt(b"same data").unwrap();
        assert_ne!(e1.nonce, e2.nonce);
    }

    #[test]
    fn empty_plaintext_round_trip() {
        let key = test_key();
        let encrypted = key.encrypt(b"").unwrap();
        let decrypted = key.decrypt(&encrypted).unwrap();
        assert!(decrypted.is_empty());
    }

    #[test]
    fn file_encrypt_decrypt_round_trip() {
        let key = test_key();
        let dir = tempfile::tempdir().unwrap();
        let plain_path = dir.path().join("plain.txt");
        let enc_path = dir.path().join("encrypted.bin");

        let original = b"Medical record content for file encryption test";
        std::fs::write(&plain_path, original).unwrap();

        encrypt_file(&key, &plain_path, &enc_path).unwrap();

        // Encrypted file should differ from plaintext
        let enc_bytes = std::fs::read(&enc_path).unwrap();
        assert_ne!(&enc_bytes, original.as_slice());

        let decrypted = decrypt_file(&key, &enc_path).unwrap();
        assert_eq!(&decrypted, original);
    }

    // ── Version header tests (A.2) ─────────────────────────────────

    #[test]
    fn to_bytes_starts_with_magic_and_version() {
        let key = test_key();
        let encrypted = key.encrypt(b"version test").unwrap();
        let bytes = encrypted.to_bytes();
        assert_eq!(bytes[0], ENCRYPTED_MAGIC, "First byte must be magic marker 0xCE");
        assert_eq!(bytes[1], CURRENT_VERSION, "Second byte must be version 0x01");
        // Total: 2 (magic+version) + 12 (nonce) + ciphertext
        assert_eq!(bytes.len(), 2 + NONCE_LENGTH + encrypted.ciphertext.len());
    }

    #[test]
    fn from_bytes_parses_versioned_format() {
        let key = test_key();
        let encrypted = key.encrypt(b"versioned data").unwrap();
        let bytes = encrypted.to_bytes();
        assert_eq!(bytes[0], ENCRYPTED_MAGIC);
        assert_eq!(bytes[1], CURRENT_VERSION);
        let restored = EncryptedData::from_bytes(&bytes).unwrap();
        let decrypted = key.decrypt(&restored).unwrap();
        assert_eq!(&decrypted, b"versioned data");
    }

    #[test]
    fn from_bytes_handles_legacy_unversioned_format() {
        let key = test_key();
        let encrypted = key.encrypt(b"legacy data").unwrap();
        // Build legacy format: [nonce][ciphertext] (no magic/version prefix)
        let mut legacy_bytes = Vec::with_capacity(NONCE_LENGTH + encrypted.ciphertext.len());
        legacy_bytes.extend_from_slice(&encrypted.nonce);
        legacy_bytes.extend_from_slice(&encrypted.ciphertext);
        // Parse legacy format — must succeed
        let restored = EncryptedData::from_bytes(&legacy_bytes).unwrap();
        let decrypted = key.decrypt(&restored).unwrap();
        assert_eq!(&decrypted, b"legacy data");
    }

    #[test]
    fn from_bytes_rejects_unknown_version() {
        // Build bytes with magic 0xCE but unknown version 0xFF
        let mut bytes = vec![ENCRYPTED_MAGIC, 0xFF];
        bytes.extend_from_slice(&[0u8; NONCE_LENGTH]);
        bytes.extend_from_slice(&[0u8; 32]); // fake ciphertext
        let result = EncryptedData::from_bytes(&bytes);
        assert!(result.is_err(), "Unknown version should be rejected");
    }
}
