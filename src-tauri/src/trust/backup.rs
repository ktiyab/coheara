use std::io::{Read, Write};
use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::crypto::encryption::EncryptedData;
use crate::crypto::keys::{ProfileKey, SALT_LENGTH};
use crate::crypto::profile::ProfileSession;

use super::fs_helpers::{calculate_dir_size, find_latest_backup};
use super::TrustError;

/// Magic bytes for .coheara-backup files.
const BACKUP_MAGIC: &[u8; 8] = b"COHEARA\x01";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResult {
    pub backup_path: String,
    pub total_documents: u32,
    pub total_size_bytes: u64,
    pub created_at: String,
    pub encrypted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub version: u32,
    pub created_at: String,
    pub profile_name: String,
    pub document_count: u32,
    pub coheara_version: String,
    /// Base64-encoded salt for key derivation during restore.
    pub salt_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePreview {
    pub metadata: BackupMetadata,
    pub file_count: u32,
    pub total_size_bytes: u64,
    pub compatible: bool,
    pub compatibility_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub documents_restored: u32,
    pub total_size_bytes: u64,
    pub warnings: Vec<String>,
}

/// Create an encrypted backup of the entire profile.
///
/// The backup includes: SQLite DB, vectors, originals, markdown, exports.
/// Accepts a `ProfileSession` for encryption access and a `ProfileKey` for
/// test scenarios where a session isn't available.
pub fn create_backup_with_key(
    profile_dir: &Path,
    profile_name: &str,
    encrypt_fn: &dyn Fn(&[u8]) -> Result<EncryptedData, crate::crypto::CryptoError>,
    output_path: &Path,
    db_key: Option<&[u8; 32]>,
) -> Result<BackupResult, TrustError> {
    let db_path = profile_dir.join("database/coheara.db");
    let conn = crate::db::sqlite::open_database(&db_path, db_key)?;

    let doc_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM documents",
        [],
        |row| row.get(0),
    )?;

    // Read salt for inclusion in metadata
    let salt_path = profile_dir.join("salt.bin");
    let salt_bytes = std::fs::read(&salt_path)?;
    let salt_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &salt_bytes,
    );

    let now = chrono::Local::now().naive_local();
    let metadata = BackupMetadata {
        version: 1,
        created_at: now.to_string(),
        profile_name: profile_name.into(),
        document_count: doc_count,
        coheara_version: env!("CARGO_PKG_VERSION").into(),
        salt_b64,
    };

    // 1. Create tar.gz archive in memory
    let mut tar_bytes = Vec::new();
    {
        let gz = flate2::write::GzEncoder::new(&mut tar_bytes, flate2::Compression::default());
        let mut tar = tar::Builder::new(gz);

        if db_path.exists() {
            tar.append_path_with_name(&db_path, "database/coheara.db")?;
        }

        let dirs_to_backup = ["vectors", "originals", "markdown", "exports"];
        for dir_name in &dirs_to_backup {
            let dir_path = profile_dir.join(dir_name);
            if dir_path.exists() && dir_path.is_dir() {
                tar.append_dir_all(*dir_name, &dir_path)?;
            }
        }

        tar.into_inner()?.finish()?;
    }

    // 2. Encrypt the tar.gz
    let encrypted = encrypt_fn(&tar_bytes)?;
    let encrypted_bytes = encrypted.to_bytes();

    // 3. Write backup file: magic + metadata_len + metadata_json + encrypted_payload
    let metadata_json = serde_json::to_vec(&metadata)?;
    let metadata_len = (metadata_json.len() as u32).to_le_bytes();

    let mut file = std::fs::File::create(output_path)?;
    file.write_all(BACKUP_MAGIC)?;
    file.write_all(&metadata_len)?;
    file.write_all(&metadata_json)?;
    file.write_all(&encrypted_bytes)?;
    file.flush()?;

    let total_size = std::fs::metadata(output_path)?.len();

    tracing::info!(
        documents = doc_count,
        size_bytes = total_size,
        "Backup created"
    );

    Ok(BackupResult {
        backup_path: output_path.to_string_lossy().into_owned(),
        total_documents: doc_count,
        total_size_bytes: total_size,
        created_at: now.to_string(),
        encrypted: true,
    })
}

/// Create an encrypted backup using a ProfileSession.
pub fn create_backup(
    session: &ProfileSession,
    output_path: &Path,
) -> Result<BackupResult, TrustError> {
    let profile_dir = session
        .db_path()
        .parent()
        .and_then(|db_dir| db_dir.parent())
        .ok_or_else(|| TrustError::Validation("Cannot determine profile directory".into()))?;

    create_backup_with_key(
        profile_dir,
        &session.profile_name,
        &|plaintext| session.encrypt(plaintext),
        output_path,
        Some(session.key_bytes()),
    )
}

/// Preview a backup file — reads unencrypted metadata only.
pub fn preview_backup(backup_path: &Path) -> Result<RestorePreview, TrustError> {
    let mut file = std::fs::File::open(backup_path)?;

    // Read and verify magic bytes
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic)?;
    if &magic != BACKUP_MAGIC {
        return Err(TrustError::Validation(
            "Not a valid Coheara backup file".into(),
        ));
    }

    // Read metadata length (4 bytes LE)
    let mut len_bytes = [0u8; 4];
    file.read_exact(&mut len_bytes)?;
    let metadata_len = u32::from_le_bytes(len_bytes) as usize;

    if metadata_len > 10_000_000 {
        return Err(TrustError::Validation(
            "Backup metadata too large — file may be corrupted".into(),
        ));
    }

    // Read metadata JSON
    let mut metadata_bytes = vec![0u8; metadata_len];
    file.read_exact(&mut metadata_bytes)?;
    let metadata: BackupMetadata = serde_json::from_slice(&metadata_bytes)?;

    let file_size = std::fs::metadata(backup_path)?.len();

    let compatible = metadata.version <= 1;
    let compat_msg = if !compatible {
        Some("This backup was created by a newer version of Coheara.".into())
    } else {
        None
    };

    Ok(RestorePreview {
        metadata,
        file_count: 0, // Cannot count without decryption
        total_size_bytes: file_size,
        compatible,
        compatibility_message: compat_msg,
    })
}

/// Restore a backup — decrypts and extracts to target directory.
pub fn restore_backup(
    backup_path: &Path,
    password: &str,
    target_dir: &Path,
) -> Result<RestoreResult, TrustError> {
    let mut file = std::fs::File::open(backup_path)?;

    // Read magic
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic)?;
    if &magic != BACKUP_MAGIC {
        return Err(TrustError::Validation(
            "Not a valid Coheara backup file".into(),
        ));
    }

    // Read metadata
    let mut len_bytes = [0u8; 4];
    file.read_exact(&mut len_bytes)?;
    let metadata_len = u32::from_le_bytes(len_bytes) as usize;
    let mut metadata_bytes = vec![0u8; metadata_len];
    file.read_exact(&mut metadata_bytes)?;
    let metadata: BackupMetadata = serde_json::from_slice(&metadata_bytes)?;

    // Decode salt from metadata
    let salt_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &metadata.salt_b64,
    )
    .map_err(|e| TrustError::Validation(format!("Invalid salt in backup: {e}")))?;

    if salt_bytes.len() != SALT_LENGTH {
        return Err(TrustError::Validation("Invalid salt length in backup".into()));
    }

    let mut salt = [0u8; SALT_LENGTH];
    salt.copy_from_slice(&salt_bytes);

    // Derive key from password + salt
    let key = ProfileKey::derive(password, &salt);

    // Read encrypted payload
    let mut encrypted_bytes = Vec::new();
    file.read_to_end(&mut encrypted_bytes)?;

    if encrypted_bytes.is_empty() {
        return Err(TrustError::Validation("Backup file is empty or truncated".into()));
    }

    // Decrypt
    let encrypted = EncryptedData::from_bytes(&encrypted_bytes)
        .map_err(|_| TrustError::Crypto("Failed to parse encrypted payload".into()))?;
    let tar_gz_bytes = key
        .decrypt(&encrypted)
        .map_err(|_| TrustError::Crypto("Incorrect password or corrupted backup".into()))?;

    // Extract tar.gz to target directory
    std::fs::create_dir_all(target_dir)?;
    let gz = flate2::read::GzDecoder::new(&tar_gz_bytes[..]);
    let mut archive = tar::Archive::new(gz);
    archive.unpack(target_dir)?;

    // Verify restored database
    let db_path = target_dir.join("database/coheara.db");
    let mut warnings = Vec::new();
    let doc_count = if db_path.exists() {
        let conn = crate::db::sqlite::open_database(&db_path, Some(key.as_bytes()))?;
        conn.query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))?
    } else {
        warnings.push("Database file not found in backup".into());
        0
    };

    let total_size = calculate_dir_size(target_dir);

    tracing::info!(
        documents_restored = doc_count,
        size_bytes = total_size,
        "Backup restored"
    );

    Ok(RestoreResult {
        documents_restored: doc_count,
        total_size_bytes: total_size,
        warnings,
    })
}

/// Gather privacy-verifiable information about the current profile.
pub fn get_privacy_info(
    conn: &Connection,
    profile_dir: &Path,
) -> Result<PrivacyInfo, TrustError> {
    let total_size = calculate_dir_size(profile_dir);

    let doc_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM documents",
        [],
        |row| row.get(0),
    )?;

    // Find most recent .coheara-backup file in profile dir's parent (profiles/)
    let last_backup = profile_dir
        .parent()
        .and_then(|parent| find_latest_backup(parent).ok())
        .flatten();

    Ok(PrivacyInfo {
        data_location: profile_dir.to_string_lossy().into_owned(),
        total_data_size_bytes: total_size,
        document_count: doc_count,
        last_backup_date: last_backup.map(|d| d.to_string()),
        encryption_algorithm: "AES-256-GCM".into(),
        key_derivation: "PBKDF2 with 600,000 iterations".into(),
        network_permissions: "None — Coheara works fully offline".into(),
        telemetry: "None — no analytics, no tracking, no crash reporting".into(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyInfo {
    pub data_location: String,
    pub total_data_size_bytes: u64,
    pub document_count: u32,
    pub last_backup_date: Option<String>,
    pub encryption_algorithm: String,
    pub key_derivation: String,
    pub network_permissions: String,
    pub telemetry: String,
}
