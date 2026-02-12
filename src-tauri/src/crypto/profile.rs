use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::encryption::EncryptedData;
use super::keys::{generate_salt, ProfileKey, SALT_LENGTH};
use super::recovery::RecoveryPhrase;
use super::CryptoError;
use crate::db::sqlite;

const VERIFICATION_PLAINTEXT: &[u8] = b"COHEARA_PROFILE_VERIFICATION_V1";

/// Profile metadata (stored unencrypted — names are visible by design)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileInfo {
    pub id: Uuid,
    pub name: String,
    pub created_at: NaiveDateTime,
    pub managed_by: Option<String>,
    pub password_hint: Option<String>,
}

/// Active profile session — holds derived key in memory.
/// Key is zeroed when session is dropped.
pub struct ProfileSession {
    pub profile_id: Uuid,
    pub profile_name: String,
    key: ProfileKey,
    pub db_path: PathBuf,
}

impl ProfileSession {
    /// Encrypt data using this profile's key
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedData, CryptoError> {
        self.key.encrypt(plaintext)
    }

    /// Decrypt data using this profile's key
    pub fn decrypt(&self, encrypted: &EncryptedData) -> Result<Vec<u8>, CryptoError> {
        self.key.decrypt(encrypted)
    }

    /// Get the SQLite database path for this profile
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

impl Drop for ProfileSession {
    fn drop(&mut self) {
        tracing::info!(profile_id = %self.profile_id, "Profile session closed, key zeroed");
    }
}

/// Create a new profile. Returns profile info and recovery phrase.
pub fn create_profile(
    profiles_dir: &Path,
    name: &str,
    password: &str,
    managed_by: Option<&str>,
) -> Result<(ProfileInfo, RecoveryPhrase), CryptoError> {
    // Check for duplicate name
    let existing = list_profiles(profiles_dir)?;
    if existing.iter().any(|p| p.name == name) {
        return Err(CryptoError::ProfileExists(name.to_string()));
    }

    let profile_id = Uuid::new_v4();
    let profile_dir = profiles_dir.join(profile_id.to_string());

    // Create directory structure
    std::fs::create_dir_all(profile_dir.join("database"))?;
    std::fs::create_dir_all(profile_dir.join("vectors"))?;
    std::fs::create_dir_all(profile_dir.join("originals"))?;
    std::fs::create_dir_all(profile_dir.join("markdown"))?;
    std::fs::create_dir_all(profile_dir.join("exports"))?;

    // Generate cryptographic material
    let salt = generate_salt();
    let recovery_salt = generate_salt();
    let master_key = ProfileKey::derive(password, &salt);
    let recovery_phrase = RecoveryPhrase::generate();
    let recovery_key =
        ProfileKey::derive_from_recovery(recovery_phrase.as_str(), &recovery_salt)?;

    // Store salts
    std::fs::write(profile_dir.join("salt.bin"), salt)?;
    std::fs::write(profile_dir.join("recovery_salt.bin"), recovery_salt)?;

    // Store password verification token
    let verification = master_key.encrypt(VERIFICATION_PLAINTEXT)?;
    std::fs::write(profile_dir.join("verification.enc"), verification.to_bytes())?;

    // Store recovery blob: master key encrypted with recovery-derived key
    let recovery_blob = recovery_key.encrypt(master_key.as_bytes())?;
    std::fs::write(
        profile_dir.join("recovery_blob.enc"),
        recovery_blob.to_bytes(),
    )?;

    // Initialize SQLite database
    let db_path = profile_dir.join("database/coheara.db");
    let conn = sqlite::open_database(&db_path).map_err(CryptoError::Database)?;
    drop(conn);

    // Save profile info
    let info = ProfileInfo {
        id: profile_id,
        name: name.to_string(),
        created_at: chrono::Local::now().naive_local(),
        managed_by: managed_by.map(|s| s.to_string()),
        password_hint: None,
    };
    save_profile_info(profiles_dir, &info)?;

    tracing::info!(profile_id = %profile_id, "Profile created");
    Ok((info, recovery_phrase))
}

/// Open existing profile with password
pub fn open_profile(
    profiles_dir: &Path,
    profile_id: &Uuid,
    password: &str,
) -> Result<ProfileSession, CryptoError> {
    let profile_dir = profiles_dir.join(profile_id.to_string());
    if !profile_dir.exists() {
        return Err(CryptoError::ProfileNotFound(*profile_id));
    }

    // Load salt
    let salt = load_salt(&profile_dir.join("salt.bin"))?;

    // Derive key
    let key = ProfileKey::derive(password, &salt);

    // Verify password
    let verification_bytes = std::fs::read(profile_dir.join("verification.enc"))?;
    let verification = EncryptedData::from_bytes(&verification_bytes)?;
    if !verify_password(&key, &verification) {
        return Err(CryptoError::WrongPassword);
    }

    // Load profile info
    let info = load_profile_info(profiles_dir, profile_id)?;

    Ok(ProfileSession {
        profile_id: *profile_id,
        profile_name: info.name,
        key,
        db_path: profile_dir.join("database/coheara.db"),
    })
}

/// Recover profile using BIP39 recovery phrase
pub fn recover_profile(
    profiles_dir: &Path,
    profile_id: &Uuid,
    recovery_phrase: &str,
) -> Result<ProfileSession, CryptoError> {
    let profile_dir = profiles_dir.join(profile_id.to_string());
    if !profile_dir.exists() {
        return Err(CryptoError::ProfileNotFound(*profile_id));
    }

    // Load recovery salt
    let recovery_salt = load_salt(&profile_dir.join("recovery_salt.bin"))?;

    // Derive recovery key
    let recovery_key = ProfileKey::derive_from_recovery(recovery_phrase, &recovery_salt)?;

    // Decrypt recovery blob to get master key bytes
    let recovery_blob_bytes = std::fs::read(profile_dir.join("recovery_blob.enc"))?;
    let recovery_blob = EncryptedData::from_bytes(&recovery_blob_bytes)?;
    let master_key_bytes = recovery_key.decrypt(&recovery_blob)?;

    if master_key_bytes.len() != 32 {
        return Err(CryptoError::CorruptedProfile);
    }

    let mut key_array = [0u8; 32];
    key_array.copy_from_slice(&master_key_bytes);
    let master_key = ProfileKey::from_bytes_internal(key_array);

    // Verify the recovered key works
    let verification_bytes = std::fs::read(profile_dir.join("verification.enc"))?;
    let verification = EncryptedData::from_bytes(&verification_bytes)?;
    if !verify_password(&master_key, &verification) {
        return Err(CryptoError::CorruptedProfile);
    }

    let info = load_profile_info(profiles_dir, profile_id)?;

    tracing::info!(profile_id = %profile_id, "Profile recovered via recovery phrase");
    Ok(ProfileSession {
        profile_id: *profile_id,
        profile_name: info.name,
        key: master_key,
        db_path: profile_dir.join("database/coheara.db"),
    })
}

/// List available profiles (names and IDs only)
pub fn list_profiles(profiles_dir: &Path) -> Result<Vec<ProfileInfo>, CryptoError> {
    let profiles_file = profiles_dir.join("profiles.json");
    if !profiles_file.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&profiles_file)?;
    let profiles: Vec<ProfileInfo> =
        serde_json::from_str(&content).map_err(|_| CryptoError::CorruptedProfile)?;
    Ok(profiles)
}

/// Cryptographic erasure: overwrite key material, delete profile directory
pub fn delete_profile(profiles_dir: &Path, profile_id: &Uuid) -> Result<(), CryptoError> {
    let profile_dir = profiles_dir.join(profile_id.to_string());
    if !profile_dir.exists() {
        return Err(CryptoError::ProfileNotFound(*profile_id));
    }

    // Step 1: Overwrite salt files with random data (destroys key derivation path)
    let random_salt = generate_salt();
    let _ = std::fs::write(profile_dir.join("salt.bin"), random_salt);
    let _ = std::fs::write(profile_dir.join("recovery_salt.bin"), random_salt);

    // Step 2: Overwrite verification and recovery blob
    let random_data = vec![0u8; 256];
    let _ = std::fs::write(profile_dir.join("verification.enc"), &random_data);
    let _ = std::fs::write(profile_dir.join("recovery_blob.enc"), &random_data);

    // Step 3: Delete the entire profile directory
    std::fs::remove_dir_all(&profile_dir)?;

    // Step 4: Remove from profiles.json
    remove_profile_info(profiles_dir, profile_id)?;

    tracing::info!(profile_id = %profile_id, "Profile cryptographically erased");
    Ok(())
}

// ═══════════════════════════════════════════
// Internal helpers
// ═══════════════════════════════════════════

fn load_salt(path: &Path) -> Result<[u8; SALT_LENGTH], CryptoError> {
    let bytes = std::fs::read(path)?;
    bytes
        .try_into()
        .map_err(|_| CryptoError::CorruptedProfile)
}

fn verify_password(key: &ProfileKey, stored: &EncryptedData) -> bool {
    match key.decrypt(stored) {
        Ok(plaintext) => plaintext == VERIFICATION_PLAINTEXT,
        Err(_) => false,
    }
}

fn save_profile_info(profiles_dir: &Path, info: &ProfileInfo) -> Result<(), CryptoError> {
    let mut profiles = list_profiles(profiles_dir)?;
    profiles.push(info.clone());
    let json = serde_json::to_string_pretty(&profiles)
        .map_err(|_| CryptoError::CorruptedProfile)?;
    std::fs::write(profiles_dir.join("profiles.json"), json)?;
    Ok(())
}

fn load_profile_info(
    profiles_dir: &Path,
    profile_id: &Uuid,
) -> Result<ProfileInfo, CryptoError> {
    let profiles = list_profiles(profiles_dir)?;
    profiles
        .into_iter()
        .find(|p| p.id == *profile_id)
        .ok_or(CryptoError::ProfileNotFound(*profile_id))
}

fn remove_profile_info(profiles_dir: &Path, profile_id: &Uuid) -> Result<(), CryptoError> {
    let profiles = list_profiles(profiles_dir)?;
    let filtered: Vec<_> = profiles.into_iter().filter(|p| p.id != *profile_id).collect();
    let json = serde_json::to_string_pretty(&filtered)
        .map_err(|_| CryptoError::CorruptedProfile)?;
    std::fs::write(profiles_dir.join("profiles.json"), json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn create_profile_creates_directory_structure() {
        let dir = test_dir();
        let (info, phrase) = create_profile(dir.path(), "Alice", "strong_password_123", None).unwrap();

        assert_eq!(info.name, "Alice");
        assert_eq!(phrase.words().len(), 12);

        let profile_dir = dir.path().join(info.id.to_string());
        assert!(profile_dir.join("salt.bin").exists());
        assert!(profile_dir.join("recovery_salt.bin").exists());
        assert!(profile_dir.join("verification.enc").exists());
        assert!(profile_dir.join("recovery_blob.enc").exists());
        assert!(profile_dir.join("database/coheara.db").exists());
        assert!(profile_dir.join("vectors").exists());
        assert!(profile_dir.join("originals").exists());
        assert!(profile_dir.join("markdown").exists());
        assert!(profile_dir.join("exports").exists());
    }

    #[test]
    fn open_with_correct_password() {
        let dir = test_dir();
        let (info, _phrase) = create_profile(dir.path(), "Bob", "correct_pass", None).unwrap();

        let session = open_profile(dir.path(), &info.id, "correct_pass").unwrap();
        assert_eq!(session.profile_name, "Bob");

        // Session can encrypt/decrypt
        let encrypted = session.encrypt(b"medical data").unwrap();
        let decrypted = session.decrypt(&encrypted).unwrap();
        assert_eq!(&decrypted, b"medical data");
    }

    #[test]
    fn open_with_wrong_password_fails() {
        let dir = test_dir();
        let (info, _phrase) = create_profile(dir.path(), "Carol", "real_pass", None).unwrap();

        let result = open_profile(dir.path(), &info.id, "wrong_pass");
        assert!(matches!(result, Err(CryptoError::WrongPassword)));
    }

    #[test]
    fn recover_with_correct_phrase() {
        let dir = test_dir();
        let (info, phrase) = create_profile(dir.path(), "Dave", "password123", None).unwrap();
        let phrase_str = phrase.as_str().to_string();
        drop(phrase);

        let session = recover_profile(dir.path(), &info.id, &phrase_str).unwrap();
        assert_eq!(session.profile_name, "Dave");

        // Recovered session can encrypt/decrypt same as password-opened session
        let encrypted = session.encrypt(b"recovery test").unwrap();
        let decrypted = session.decrypt(&encrypted).unwrap();
        assert_eq!(&decrypted, b"recovery test");
    }

    #[test]
    fn recover_with_wrong_phrase_fails() {
        let dir = test_dir();
        let (info, _phrase) = create_profile(dir.path(), "Eve", "password", None).unwrap();

        // Generate a different valid phrase
        let wrong_phrase = RecoveryPhrase::generate();
        let result = recover_profile(dir.path(), &info.id, wrong_phrase.as_str());
        // Will fail at decryption (wrong key) or verification
        assert!(result.is_err());
    }

    #[test]
    fn list_profiles_returns_all() {
        let dir = test_dir();
        create_profile(dir.path(), "Profile1", "pass1", None).unwrap();
        create_profile(dir.path(), "Profile2", "pass2", None).unwrap();
        create_profile(dir.path(), "Profile3", "pass3", Some("Caregiver")).unwrap();

        let profiles = list_profiles(dir.path()).unwrap();
        assert_eq!(profiles.len(), 3);

        let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"Profile1"));
        assert!(names.contains(&"Profile2"));
        assert!(names.contains(&"Profile3"));
    }

    #[test]
    fn delete_profile_removes_all_files() {
        let dir = test_dir();
        let (info, _phrase) = create_profile(dir.path(), "ToDelete", "pass", None).unwrap();

        let profile_dir = dir.path().join(info.id.to_string());
        assert!(profile_dir.exists());

        delete_profile(dir.path(), &info.id).unwrap();

        assert!(!profile_dir.exists());
    }

    #[test]
    fn delete_profile_removes_from_list() {
        let dir = test_dir();
        let (info1, _) = create_profile(dir.path(), "Keep", "pass1", None).unwrap();
        let (info2, _) = create_profile(dir.path(), "Delete", "pass2", None).unwrap();

        delete_profile(dir.path(), &info2.id).unwrap();

        let profiles = list_profiles(dir.path()).unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].id, info1.id);
    }

    #[test]
    fn deleted_profile_cannot_be_opened() {
        let dir = test_dir();
        let (info, _phrase) = create_profile(dir.path(), "Erased", "password", None).unwrap();

        delete_profile(dir.path(), &info.id).unwrap();

        let result = open_profile(dir.path(), &info.id, "password");
        assert!(matches!(result, Err(CryptoError::ProfileNotFound(_))));
    }

    #[test]
    fn duplicate_profile_name_rejected() {
        let dir = test_dir();
        create_profile(dir.path(), "Unique", "pass1", None).unwrap();
        let result = create_profile(dir.path(), "Unique", "pass2", None);
        assert!(matches!(result, Err(CryptoError::ProfileExists(_))));
    }

    #[test]
    fn open_nonexistent_profile_fails() {
        let dir = test_dir();
        let fake_id = Uuid::new_v4();
        let result = open_profile(dir.path(), &fake_id, "password");
        assert!(matches!(result, Err(CryptoError::ProfileNotFound(_))));
    }

    #[test]
    fn profile_database_is_initialized() {
        let dir = test_dir();
        let (info, _phrase) = create_profile(dir.path(), "DbTest", "pass", None).unwrap();

        let session = open_profile(dir.path(), &info.id, "pass").unwrap();
        let conn = rusqlite::Connection::open(session.db_path()).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(count >= 18, "Expected at least 18 tables, got {count}");
    }

    #[test]
    fn managed_by_attribution_stored() {
        let dir = test_dir();
        let (info, _phrase) = create_profile(dir.path(), "Patient", "pass", Some("Dr. Smith")).unwrap();
        assert_eq!(info.managed_by.as_deref(), Some("Dr. Smith"));

        let profiles = list_profiles(dir.path()).unwrap();
        assert_eq!(profiles[0].managed_by.as_deref(), Some("Dr. Smith"));
    }
}
