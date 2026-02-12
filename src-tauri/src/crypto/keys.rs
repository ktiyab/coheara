use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use zeroize::Zeroize;

use super::CryptoError;
use super::encryption::EncryptedData;

pub const PBKDF2_ITERATIONS: u32 = 600_000;
pub const KEY_LENGTH: usize = 32; // AES-256
pub const SALT_LENGTH: usize = 32;

/// Master encryption key — zeroed on drop
#[derive(Zeroize)]
#[zeroize(drop)]
pub struct ProfileKey {
    pub(super) key_bytes: [u8; KEY_LENGTH],
}

impl ProfileKey {
    /// Derive from password + salt using PBKDF2-SHA256
    pub fn derive(password: &str, salt: &[u8; SALT_LENGTH]) -> Self {
        let mut key_bytes = [0u8; KEY_LENGTH];
        pbkdf2_hmac::<Sha256>(
            password.as_bytes(),
            salt,
            PBKDF2_ITERATIONS,
            &mut key_bytes,
        );
        Self { key_bytes }
    }

    /// Access the raw key bytes (internal use only)
    pub(crate) fn as_bytes(&self) -> &[u8; KEY_LENGTH] {
        &self.key_bytes
    }

    /// Encrypt data using AES-256-GCM
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedData, CryptoError> {
        EncryptedData::encrypt(&self.key_bytes, plaintext)
    }

    /// Decrypt data using AES-256-GCM
    pub fn decrypt(&self, encrypted: &EncryptedData) -> Result<Vec<u8>, CryptoError> {
        encrypted.decrypt(&self.key_bytes)
    }
}

/// Generate a cryptographically random salt
pub fn generate_salt() -> [u8; SALT_LENGTH] {
    use rand::RngCore;
    let mut salt = [0u8; SALT_LENGTH];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_produces_deterministic_key() {
        let salt = [42u8; SALT_LENGTH];
        let key1 = ProfileKey::derive("password", &salt);
        let key2 = ProfileKey::derive("password", &salt);
        assert_eq!(key1.key_bytes, key2.key_bytes);
    }

    #[test]
    fn different_passwords_produce_different_keys() {
        let salt = [42u8; SALT_LENGTH];
        let key1 = ProfileKey::derive("password1", &salt);
        let key2 = ProfileKey::derive("password2", &salt);
        assert_ne!(key1.key_bytes, key2.key_bytes);
    }

    #[test]
    fn different_salts_produce_different_keys() {
        let salt1 = [1u8; SALT_LENGTH];
        let salt2 = [2u8; SALT_LENGTH];
        let key1 = ProfileKey::derive("password", &salt1);
        let key2 = ProfileKey::derive("password", &salt2);
        assert_ne!(key1.key_bytes, key2.key_bytes);
    }

    #[test]
    fn generate_salt_is_random() {
        let s1 = generate_salt();
        let s2 = generate_salt();
        assert_ne!(s1, s2);
    }

    #[test]
    fn pbkdf2_takes_meaningful_time() {
        let start = std::time::Instant::now();
        let _key = ProfileKey::derive("test_password", &[0u8; SALT_LENGTH]);
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() > 100,
            "PBKDF2 too fast: {}ms — brute force protection insufficient",
            elapsed.as_millis()
        );
    }
}
