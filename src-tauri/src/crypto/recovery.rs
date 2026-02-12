use bip39::{Language, Mnemonic};
use rand::RngCore;
use zeroize::Zeroize;

use super::keys::{ProfileKey, PBKDF2_ITERATIONS, KEY_LENGTH, SALT_LENGTH};
use super::CryptoError;

/// Recovery phrase wrapper — zeroed on drop
#[derive(Zeroize)]
#[zeroize(drop)]
pub struct RecoveryPhrase {
    phrase: String,
}

impl RecoveryPhrase {
    /// Generate a new 12-word BIP39 recovery phrase
    /// 16 bytes entropy → 12 words (128-bit security)
    pub fn generate() -> Self {
        let mut entropy = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut entropy);
        let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
            .expect("16 bytes is valid BIP39 entropy");
        Self {
            phrase: mnemonic.to_string(),
        }
    }

    /// Validate a recovery phrase against BIP39 wordlist
    pub fn validate(phrase: &str) -> bool {
        Mnemonic::parse_in_normalized(Language::English, phrase).is_ok()
    }

    /// Access the phrase as a string slice
    pub fn as_str(&self) -> &str {
        &self.phrase
    }

    /// Get individual words
    pub fn words(&self) -> Vec<&str> {
        self.phrase.split_whitespace().collect()
    }
}

impl ProfileKey {
    /// Derive key from recovery phrase + recovery salt
    pub fn derive_from_recovery(
        phrase: &str,
        recovery_salt: &[u8; SALT_LENGTH],
    ) -> Result<Self, CryptoError> {
        if !RecoveryPhrase::validate(phrase) {
            return Err(CryptoError::InvalidRecoveryPhrase);
        }

        let mut key_bytes = [0u8; KEY_LENGTH];
        pbkdf2::pbkdf2_hmac::<sha2::Sha256>(
            phrase.as_bytes(),
            recovery_salt,
            PBKDF2_ITERATIONS,
            &mut key_bytes,
        );
        Ok(Self::from_bytes_internal(key_bytes))
    }

    /// Construct from raw bytes (internal use for recovery)
    pub(crate) fn from_bytes_internal(key_bytes: [u8; KEY_LENGTH]) -> Self {
        Self { key_bytes }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_12_words() {
        let phrase = RecoveryPhrase::generate();
        assert_eq!(phrase.words().len(), 12);
    }

    #[test]
    fn generated_phrase_is_valid_bip39() {
        let phrase = RecoveryPhrase::generate();
        assert!(RecoveryPhrase::validate(phrase.as_str()));
    }

    #[test]
    fn invalid_phrase_rejected() {
        assert!(!RecoveryPhrase::validate("not a valid recovery phrase"));
        assert!(!RecoveryPhrase::validate(""));
        assert!(!RecoveryPhrase::validate("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon")); // wrong checksum but valid words - actually this might pass
    }

    #[test]
    fn derive_from_recovery_rejects_invalid_phrase() {
        let salt = [0u8; SALT_LENGTH];
        let result = ProfileKey::derive_from_recovery("invalid phrase here", &salt);
        assert!(matches!(result, Err(CryptoError::InvalidRecoveryPhrase)));
    }

    #[test]
    fn derive_from_recovery_is_deterministic() {
        let phrase = RecoveryPhrase::generate();
        let salt = [42u8; SALT_LENGTH];
        let key1 = ProfileKey::derive_from_recovery(phrase.as_str(), &salt).unwrap();
        let key2 = ProfileKey::derive_from_recovery(phrase.as_str(), &salt).unwrap();
        assert_eq!(key1.as_bytes(), key2.as_bytes());
    }

    #[test]
    fn each_generation_produces_unique_phrase() {
        let p1 = RecoveryPhrase::generate();
        let p2 = RecoveryPhrase::generate();
        assert_ne!(p1.as_str(), p2.as_str());
    }
}
