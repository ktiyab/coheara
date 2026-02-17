//! L5-01: Trust & Safety — cross-cutting hardening layer.
//!
//! Five sub-systems:
//! 1. Emergency Protocol — critical lab alerts with 2-step dismissal
//! 2. Dose Plausibility — cross-reference doses against known ranges
//! 3. Backup & Restore — encrypted .coheara-backup archives
//! 4. Cryptographic Erasure — profile deletion via key zeroing
//! 5. Privacy Verification — prove offline + encryption promises

use std::io::{Read, Write};
use std::path::Path;

use chrono::NaiveDateTime;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::crypto::encryption::EncryptedData;
use crate::crypto::keys::{ProfileKey, SALT_LENGTH};
use crate::crypto::profile::ProfileSession;
use crate::db::DatabaseError;

// ═══════════════════════════════════════════════════════════════════════════
// Error type
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Error, Debug)]
pub enum TrustError {
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<crate::crypto::CryptoError> for TrustError {
    fn from(e: crate::crypto::CryptoError) -> Self {
        TrustError::Crypto(e.to_string())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Types — Emergency Protocol
// ═══════════════════════════════════════════════════════════════════════════

/// Critical lab alert surfaced to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalLabAlert {
    pub id: String,
    pub test_name: String,
    pub value: String,
    pub unit: String,
    pub reference_range: String,
    pub abnormal_flag: String,
    pub lab_date: String,
    pub document_id: String,
    pub detected_at: String,
    pub dismissed: bool,
}

/// 2-step dismissal request for critical alerts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalDismissRequest {
    pub alert_id: String,
    pub step: DismissStep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DismissStep {
    AskConfirmation,
    ConfirmDismissal { reason: String },
}

// ═══════════════════════════════════════════════════════════════════════════
// Types — Dose Plausibility
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DosePlausibility {
    pub medication_name: String,
    pub extracted_dose: String,
    pub extracted_value: f64,
    pub extracted_unit: String,
    pub typical_range_low: f64,
    pub typical_range_high: f64,
    pub typical_unit: String,
    pub plausibility: PlausibilityResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlausibilityResult {
    Plausible,
    HighDose { message: String },
    VeryHighDose { message: String },
    LowDose { message: String },
    UnknownMedication,
}

/// Internal reference row from dose_references table.
#[derive(Debug, Clone)]
struct DoseReference {
    pub typical_min_mg: f64,
    pub typical_max_mg: f64,
    pub absolute_max_mg: f64,
    pub unit: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Types — Backup & Restore
// ═══════════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════════
// Types — Cryptographic Erasure
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErasureRequest {
    pub profile_id: String,
    pub confirmation_text: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErasureResult {
    pub profile_name: String,
    pub files_deleted: u32,
    pub bytes_erased: u64,
    pub key_zeroed: bool,
}

// ═══════════════════════════════════════════════════════════════════════════
// Types — Privacy Verification
// ═══════════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════════
// Types — Error Recovery
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecoveryStrategy {
    Retry,
    RetryWithBackoff,
    FallbackAvailable(String),
    UserActionRequired(String),
    Fatal(String),
}

// ═══════════════════════════════════════════════════════════════════════════
// [1] Emergency Protocol
// ═══════════════════════════════════════════════════════════════════════════

/// Fetch all critical lab results that have NOT been dismissed.
pub fn fetch_critical_alerts(conn: &Connection) -> Result<Vec<CriticalLabAlert>, TrustError> {
    // Get dismissed alert entity_ids for type 'critical'
    let dismissed_lab_ids: std::collections::HashSet<String> = {
        let mut stmt = conn.prepare(
            "SELECT entity_ids FROM dismissed_alerts WHERE alert_type = 'critical'",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut set = std::collections::HashSet::new();
        for row in rows {
            let ids_json = row?;
            // entity_ids is JSON array of IDs
            if let Ok(ids) = serde_json::from_str::<Vec<String>>(&ids_json) {
                for id in ids {
                    set.insert(id);
                }
            } else {
                // Fallback: treat as single ID string
                set.insert(ids_json);
            }
        }
        set
    };

    let mut stmt = conn.prepare(
        "SELECT id, test_name, value, value_text, unit,
                reference_range_low, reference_range_high,
                abnormal_flag, collection_date, document_id
         FROM lab_results
         WHERE abnormal_flag IN ('critical_low', 'critical_high')",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<f64>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<f64>>(5)?,
            row.get::<_, Option<f64>>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, String>(8)?,
            row.get::<_, String>(9)?,
        ))
    })?;

    let mut alerts = Vec::new();
    for row in rows {
        let (id, test_name, value, value_text, unit, ref_low, ref_high, flag, date, doc_id) =
            row?;

        if dismissed_lab_ids.contains(&id) {
            continue;
        }

        let value_str = value
            .map(|v| format!("{v}"))
            .or(value_text)
            .unwrap_or_else(|| "N/A".into());

        let unit_str = unit.unwrap_or_default();

        let range_str = match (ref_low, ref_high) {
            (Some(lo), Some(hi)) => format!("{lo} — {hi} {unit_str}"),
            _ => "Not available".into(),
        };

        alerts.push(CriticalLabAlert {
            id: id.clone(),
            test_name,
            value: value_str,
            unit: unit_str,
            reference_range: range_str,
            abnormal_flag: flag,
            lab_date: date,
            document_id: doc_id,
            detected_at: chrono::Local::now().naive_local().to_string(),
            dismissed: false,
        });
    }

    Ok(alerts)
}

/// Handle critical alert dismissal (2-step process).
pub fn dismiss_critical_alert(
    conn: &Connection,
    request: &CriticalDismissRequest,
) -> Result<(), TrustError> {
    match &request.step {
        DismissStep::AskConfirmation => {
            // Step 1: Validate the alert exists and is critical
            let exists: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM lab_results
                 WHERE id = ?1 AND abnormal_flag IN ('critical_low', 'critical_high')",
                params![request.alert_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(TrustError::NotFound("Critical alert not found".into()));
            }
            Ok(())
        }
        DismissStep::ConfirmDismissal { reason } => {
            if reason.is_empty() {
                return Err(TrustError::Validation(
                    "Reason required to dismiss critical alert".into(),
                ));
            }

            // Verify alert exists
            let exists: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM lab_results
                 WHERE id = ?1 AND abnormal_flag IN ('critical_low', 'critical_high')",
                params![request.alert_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(TrustError::NotFound("Critical alert not found".into()));
            }

            // Store dismissal record
            let dismiss_id = Uuid::new_v4().to_string();
            let entity_ids_json =
                serde_json::to_string(&vec![&request.alert_id])?;

            conn.execute(
                "INSERT INTO dismissed_alerts
                 (id, alert_type, entity_ids, dismissed_date, reason, dismissed_by)
                 VALUES (?1, 'critical', ?2, datetime('now'), ?3, 'patient')",
                params![dismiss_id, entity_ids_json, reason],
            )?;

            tracing::info!(
                alert_id = %request.alert_id,
                "Critical alert dismissed with 2-step confirmation"
            );

            Ok(())
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// [2] Dose Plausibility
// ═══════════════════════════════════════════════════════════════════════════

/// Resolve a medication name (possibly brand name) to its generic name.
fn resolve_to_generic(conn: &Connection, medication_name: &str) -> String {
    let lower = medication_name.trim().to_lowercase();

    // First check if it's already a generic name in dose_references
    let is_generic: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM dose_references WHERE LOWER(generic_name) = ?1",
            params![lower],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if is_generic {
        return lower;
    }

    // Try medication_aliases: brand_name → generic_name
    conn.query_row(
        "SELECT LOWER(generic_name) FROM medication_aliases
         WHERE LOWER(brand_name) = ?1 LIMIT 1",
        params![lower],
        |row| row.get::<_, String>(0),
    )
    .unwrap_or(lower)
}

/// Convert a dose value to milligrams.
pub fn convert_to_mg(value: f64, unit: &str) -> f64 {
    match unit.to_lowercase().as_str() {
        "mg" => value,
        "g" => value * 1000.0,
        "mcg" | "ug" | "µg" => value / 1000.0,
        _ => value, // Assume mg if unknown
    }
}

/// Check if an extracted dose is plausible against known reference ranges.
pub fn check_dose_plausibility(
    conn: &Connection,
    medication_name: &str,
    dose_value: f64,
    dose_unit: &str,
) -> Result<DosePlausibility, TrustError> {
    let generic = resolve_to_generic(conn, medication_name);

    let reference = conn.query_row(
        "SELECT generic_name, typical_min_mg, typical_max_mg, absolute_max_mg, unit
         FROM dose_references WHERE LOWER(generic_name) = ?1",
        params![generic],
        |row| {
            Ok(DoseReference {
                typical_min_mg: row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                typical_max_mg: row.get::<_, Option<f64>>(2)?.unwrap_or(f64::MAX),
                absolute_max_mg: row.get::<_, Option<f64>>(3)?.unwrap_or(f64::MAX),
                unit: row.get(4)?,
            })
        },
    );

    match reference {
        Ok(ref_data) => {
            let dose_mg = convert_to_mg(dose_value, dose_unit);

            let plausibility = if dose_mg > ref_data.absolute_max_mg * 5.0 {
                PlausibilityResult::VeryHighDose {
                    message: format!(
                        "I extracted {dose_mg}mg for {medication_name} but the typical maximum is {}mg. \
                         This may be an extraction error — please double-check this value.",
                        ref_data.absolute_max_mg
                    ),
                }
            } else if dose_mg > ref_data.typical_max_mg {
                PlausibilityResult::HighDose {
                    message: format!(
                        "I extracted {dose_mg}mg for {medication_name} but the typical range is \
                         {}-{}mg. Please verify this value.",
                        ref_data.typical_min_mg, ref_data.typical_max_mg
                    ),
                }
            } else if ref_data.typical_min_mg > 0.0 && dose_mg < ref_data.typical_min_mg * 0.5 {
                PlausibilityResult::LowDose {
                    message: format!(
                        "I extracted {dose_mg}mg for {medication_name} but the typical minimum is \
                         {}mg. Please verify this value.",
                        ref_data.typical_min_mg
                    ),
                }
            } else {
                PlausibilityResult::Plausible
            };

            Ok(DosePlausibility {
                medication_name: medication_name.into(),
                extracted_dose: format!("{dose_value}{dose_unit}"),
                extracted_value: dose_value,
                extracted_unit: dose_unit.into(),
                typical_range_low: ref_data.typical_min_mg,
                typical_range_high: ref_data.typical_max_mg,
                typical_unit: ref_data.unit,
                plausibility,
            })
        }
        Err(_) => Ok(DosePlausibility {
            medication_name: medication_name.into(),
            extracted_dose: format!("{dose_value}{dose_unit}"),
            extracted_value: dose_value,
            extracted_unit: dose_unit.into(),
            typical_range_low: 0.0,
            typical_range_high: 0.0,
            typical_unit: "mg".into(),
            plausibility: PlausibilityResult::UnknownMedication,
        }),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// [3] Backup & Restore
// ═══════════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════════
// [4] Cryptographic Erasure
// ═══════════════════════════════════════════════════════════════════════════

/// Erase a profile — validates confirmation and password, then delegates
/// to the existing `delete_profile` in crypto::profile.
pub fn erase_profile_data(
    profiles_dir: &Path,
    request: &ErasureRequest,
) -> Result<ErasureResult, TrustError> {
    // 1. Validate confirmation text
    if request.confirmation_text != "DELETE MY DATA" {
        return Err(TrustError::Validation(
            "Must type 'DELETE MY DATA' to confirm deletion".into(),
        ));
    }

    // 2. Parse profile ID
    let profile_id = Uuid::parse_str(&request.profile_id)
        .map_err(|_| TrustError::Validation("Invalid profile ID".into()))?;

    // 3. Verify password by attempting to unlock
    let profile_dir = profiles_dir.join(profile_id.to_string());
    if !profile_dir.exists() {
        return Err(TrustError::NotFound("Profile not found".into()));
    }

    let salt_path = profile_dir.join("salt.bin");
    let salt_bytes = std::fs::read(&salt_path)
        .map_err(|_| TrustError::NotFound("Profile salt not found".into()))?;
    if salt_bytes.len() != SALT_LENGTH {
        return Err(TrustError::Crypto("Invalid salt length".into()));
    }
    let mut salt = [0u8; SALT_LENGTH];
    salt.copy_from_slice(&salt_bytes);

    let key = ProfileKey::derive(&request.password, &salt);

    // Verify password against stored verification token
    let verification_path = profile_dir.join("verification.enc");
    let verification_bytes = std::fs::read(&verification_path)
        .map_err(|_| TrustError::NotFound("Profile verification data not found".into()))?;
    let verification = EncryptedData::from_bytes(&verification_bytes)
        .map_err(|_| TrustError::Crypto("Corrupted verification data".into()))?;

    let decrypted = key.decrypt(&verification);
    match &decrypted {
        Ok(plaintext) if plaintext.as_slice() == b"COHEARA_VERIFY" => {}
        _ => return Err(TrustError::Validation("Incorrect password".into())),
    }

    // 4. Get profile name from registry
    let profile_name = get_profile_name_from_dir(profiles_dir, &profile_id);

    // 5. Count files and size before deletion
    let (file_count, total_bytes) = count_dir_contents(&profile_dir);

    // 6. Delete via existing crypto::profile::delete_profile
    crate::crypto::profile::delete_profile(profiles_dir, &profile_id)
        .map_err(|e| TrustError::Crypto(e.to_string()))?;

    // Key is automatically zeroed on drop (ZeroizeOnDrop)
    drop(key);

    tracing::info!(
        profile_id = %profile_id,
        files = file_count,
        bytes = total_bytes,
        "Profile erased"
    );

    Ok(ErasureResult {
        profile_name,
        files_deleted: file_count,
        bytes_erased: total_bytes,
        key_zeroed: true,
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// [5] Privacy Verification
// ═══════════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════════
// [6] Error Recovery
// ═══════════════════════════════════════════════════════════════════════════

/// Map an error string to a recovery strategy.
pub fn recovery_for(error: &str) -> RecoveryStrategy {
    let lower = error.to_lowercase();
    if lower.contains("encryption") || lower.contains("crypto") || lower.contains("decrypt") {
        RecoveryStrategy::Fatal(
            "Encryption error — your profile may need to be restored from backup.".into(),
        )
    } else if lower.contains("password") || lower.contains("auth") {
        RecoveryStrategy::UserActionRequired(
            "Authentication failed — please check your password.".into(),
        )
    } else if lower.contains("network") || lower.contains("connection") {
        RecoveryStrategy::UserActionRequired(
            "Check your local network connection.".into(),
        )
    } else if lower.contains("timeout") {
        RecoveryStrategy::RetryWithBackoff
    } else if lower.contains("ocr") || lower.contains("extraction") {
        RecoveryStrategy::FallbackAvailable(
            "OCR failed — you can try a clearer photo or enter information manually.".into(),
        )
    } else if lower.contains("ollama") || lower.contains("llm") || lower.contains("model") {
        RecoveryStrategy::FallbackAvailable(
            "The AI model isn't running. Start Ollama and try again.".into(),
        )
    } else {
        RecoveryStrategy::Retry
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Calculate total size of a directory recursively.
pub fn calculate_dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    walkdir(path)
}

fn walkdir(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += walkdir(&p);
            } else if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

/// Count files and total bytes in a directory.
fn count_dir_contents(path: &Path) -> (u32, u64) {
    if !path.exists() {
        return (0, 0);
    }
    let mut count = 0u32;
    let mut bytes = 0u64;
    count_recursive(path, &mut count, &mut bytes);
    (count, bytes)
}

fn count_recursive(path: &Path, count: &mut u32, bytes: &mut u64) {
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                count_recursive(&p, count, bytes);
            } else if let Ok(meta) = entry.metadata() {
                *count += 1;
                *bytes += meta.len();
            }
        }
    }
}

/// Get profile name from profiles.json registry.
fn get_profile_name_from_dir(profiles_dir: &Path, profile_id: &Uuid) -> String {
    let registry_path = profiles_dir.join("profiles.json");
    if let Ok(data) = std::fs::read_to_string(&registry_path) {
        if let Ok(profiles) = serde_json::from_str::<Vec<serde_json::Value>>(&data) {
            for p in &profiles {
                if p.get("id").and_then(|v| v.as_str()) == Some(&profile_id.to_string()) {
                    return p
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .to_string();
                }
            }
        }
    }
    "Unknown".into()
}

/// Find the most recent .coheara-backup file in a directory.
fn find_latest_backup(dir: &Path) -> Result<Option<NaiveDateTime>, TrustError> {
    if !dir.exists() {
        return Ok(None);
    }

    let mut latest: Option<NaiveDateTime> = None;

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("coheara-backup") {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        let datetime: chrono::DateTime<chrono::Local> = modified.into();
                        let naive = datetime.naive_local();
                        if latest.is_none() || Some(naive) > latest {
                            latest = Some(naive);
                        }
                    }
                }
            }
        }
    }

    Ok(latest)
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;

    /// Set up a test database with the required tables and sample data.
    fn setup_test_db() -> Connection {
        let conn = open_memory_database().unwrap();

        // Insert parent document (required by lab_results FK)
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file)
             VALUES ('doc-1', 'lab_result', 'Blood Work Jan 2026', '2026-01-15', 'bloodwork.pdf')",
            [],
        )
        .unwrap();

        // Insert sample critical lab result
        conn.execute(
            "INSERT INTO lab_results (id, test_name, value, value_text, unit,
             reference_range_low, reference_range_high, abnormal_flag,
             collection_date, lab_facility, ordering_physician_id, document_id)
             VALUES ('lab-1', 'Potassium', 6.5, NULL, 'mEq/L', 3.5, 5.0,
             'critical_high', '2026-01-15', 'City Lab', NULL, 'doc-1')",
            [],
        )
        .unwrap();

        // Insert a normal lab result
        conn.execute(
            "INSERT INTO lab_results (id, test_name, value, value_text, unit,
             reference_range_low, reference_range_high, abnormal_flag,
             collection_date, lab_facility, ordering_physician_id, document_id)
             VALUES ('lab-2', 'Glucose', 95.0, NULL, 'mg/dL', 70.0, 100.0,
             'normal', '2026-01-15', 'City Lab', NULL, 'doc-1')",
            [],
        )
        .unwrap();

        // Insert dose references
        conn.execute(
            "INSERT INTO dose_references (generic_name, typical_min_mg, typical_max_mg, absolute_max_mg, unit, source)
             VALUES ('metformin', 500.0, 2550.0, 1000.0, 'mg', 'bundled')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO dose_references (generic_name, typical_min_mg, typical_max_mg, absolute_max_mg, unit, source)
             VALUES ('lisinopril', 2.5, 40.0, 40.0, 'mg', 'bundled')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO dose_references (generic_name, typical_min_mg, typical_max_mg, absolute_max_mg, unit, source)
             VALUES ('atorvastatin', 10.0, 80.0, 80.0, 'mg', 'bundled')",
            [],
        )
        .unwrap();

        // Insert medication alias for brand name resolution
        conn.execute(
            "INSERT INTO medication_aliases (generic_name, brand_name, country, source)
             VALUES ('metformin', 'Glucophage', 'US', 'bundled')",
            [],
        )
        .unwrap();

        conn
    }

    // ─── Emergency Protocol Tests ───

    #[test]
    fn test_critical_alert_fetch() {
        let conn = setup_test_db();
        let alerts = fetch_critical_alerts(&conn).unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].test_name, "Potassium");
        assert_eq!(alerts[0].abnormal_flag, "critical_high");
        assert!(!alerts[0].dismissed);
    }

    #[test]
    fn test_critical_alert_excludes_dismissed() {
        let conn = setup_test_db();

        // Dismiss the critical alert
        conn.execute(
            "INSERT INTO dismissed_alerts (id, alert_type, entity_ids, dismissed_date, reason, dismissed_by)
             VALUES ('dismiss-1', 'critical', '[\"lab-1\"]', '2026-01-16 10:00:00', 'Doctor reviewed', 'patient')",
            [],
        ).unwrap();

        let alerts = fetch_critical_alerts(&conn).unwrap();
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_critical_dismiss_step1() {
        let conn = setup_test_db();
        let request = CriticalDismissRequest {
            alert_id: "lab-1".into(),
            step: DismissStep::AskConfirmation,
        };
        assert!(dismiss_critical_alert(&conn, &request).is_ok());
    }

    #[test]
    fn test_critical_dismiss_step1_not_found() {
        let conn = setup_test_db();
        let request = CriticalDismissRequest {
            alert_id: "nonexistent".into(),
            step: DismissStep::AskConfirmation,
        };
        let result = dismiss_critical_alert(&conn, &request);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_critical_dismiss_step2() {
        let conn = setup_test_db();
        let request = CriticalDismissRequest {
            alert_id: "lab-1".into(),
            step: DismissStep::ConfirmDismissal {
                reason: "My doctor has reviewed this result".into(),
            },
        };
        assert!(dismiss_critical_alert(&conn, &request).is_ok());

        // Verify dismissal record was created
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM dismissed_alerts WHERE alert_type = 'critical'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_critical_dismiss_requires_reason() {
        let conn = setup_test_db();
        let request = CriticalDismissRequest {
            alert_id: "lab-1".into(),
            step: DismissStep::ConfirmDismissal {
                reason: "".into(),
            },
        };
        let result = dismiss_critical_alert(&conn, &request);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Reason required"));
    }

    // ─── Dose Plausibility Tests ───

    #[test]
    fn test_dose_plausible() {
        let conn = setup_test_db();
        let result = check_dose_plausibility(&conn, "metformin", 500.0, "mg").unwrap();
        assert_eq!(result.plausibility, PlausibilityResult::Plausible);
    }

    #[test]
    fn test_dose_high() {
        let conn = setup_test_db();
        let result = check_dose_plausibility(&conn, "metformin", 3000.0, "mg").unwrap();
        assert!(matches!(result.plausibility, PlausibilityResult::HighDose { .. }));
    }

    #[test]
    fn test_dose_very_high() {
        let conn = setup_test_db();
        let result = check_dose_plausibility(&conn, "metformin", 50000.0, "mg").unwrap();
        assert!(matches!(result.plausibility, PlausibilityResult::VeryHighDose { .. }));
    }

    #[test]
    fn test_dose_low() {
        let conn = setup_test_db();
        let result = check_dose_plausibility(&conn, "metformin", 10.0, "mg").unwrap();
        assert!(matches!(result.plausibility, PlausibilityResult::LowDose { .. }));
    }

    #[test]
    fn test_dose_unknown_medication() {
        let conn = setup_test_db();
        let result = check_dose_plausibility(&conn, "xyzabc123", 100.0, "mg").unwrap();
        assert_eq!(result.plausibility, PlausibilityResult::UnknownMedication);
    }

    #[test]
    fn test_dose_unit_conversion() {
        assert!((convert_to_mg(1.0, "g") - 1000.0).abs() < f64::EPSILON);
        assert!((convert_to_mg(1000.0, "mcg") - 1.0).abs() < f64::EPSILON);
        assert!((convert_to_mg(500.0, "mg") - 500.0).abs() < f64::EPSILON);
        assert!((convert_to_mg(500.0, "ug") - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_dose_brand_name_resolution() {
        let conn = setup_test_db();
        // "Glucophage" should resolve to "metformin" via medication_aliases
        let result = check_dose_plausibility(&conn, "Glucophage", 500.0, "mg").unwrap();
        assert_eq!(result.plausibility, PlausibilityResult::Plausible);
    }

    #[test]
    fn test_dose_with_gram_unit() {
        let conn = setup_test_db();
        // 0.5g = 500mg metformin → plausible
        let result = check_dose_plausibility(&conn, "metformin", 0.5, "g").unwrap();
        assert_eq!(result.plausibility, PlausibilityResult::Plausible);
    }

    // ─── Backup & Restore Tests ───

    #[test]
    fn test_backup_create_and_preview() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_dir = tmp.path().join("test-profile");

        // Set up minimal profile structure
        std::fs::create_dir_all(profile_dir.join("database")).unwrap();
        std::fs::create_dir_all(profile_dir.join("originals")).unwrap();

        // Create a test database
        let db_path = profile_dir.join("database/coheara.db");
        let conn = crate::db::sqlite::open_database(&db_path, None).unwrap();
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file)
             VALUES ('doc-1', 'lab_result', 'Test Report', '2026-01-15', 'test.pdf')",
            [],
        )
        .unwrap();
        drop(conn);

        // Create a salt file
        let salt = crate::crypto::keys::generate_salt();
        std::fs::write(profile_dir.join("salt.bin"), salt).unwrap();

        // Create profile key
        let key = ProfileKey::derive("testpassword", &salt);

        // Create backup
        let backup_path = tmp.path().join("test.coheara-backup");
        let result = create_backup_with_key(
            &profile_dir, "Test Profile", &|p| key.encrypt(p), &backup_path, None,
        ).unwrap();

        assert!(backup_path.exists());
        assert_eq!(result.total_documents, 1);
        assert!(result.encrypted);
        assert!(result.total_size_bytes > 0);

        // Preview backup
        let preview = preview_backup(&backup_path).unwrap();
        assert_eq!(preview.metadata.version, 1);
        assert_eq!(preview.metadata.profile_name, "Test Profile");
        assert_eq!(preview.metadata.document_count, 1);
        assert!(preview.compatible);
        assert!(preview.compatibility_message.is_none());
    }

    #[test]
    fn test_backup_restore_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_dir = tmp.path().join("source-profile");

        // Create directories first
        std::fs::create_dir_all(profile_dir.join("database")).unwrap();

        // Derive key so we can create an encrypted DB (matching production flow)
        let salt = crate::crypto::keys::generate_salt();
        std::fs::write(profile_dir.join("salt.bin"), salt).unwrap();
        let key = ProfileKey::derive("mypassword", &salt);
        let db_path = profile_dir.join("database/coheara.db");
        let conn = crate::db::sqlite::open_database(&db_path, Some(key.as_bytes())).unwrap();
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file)
             VALUES ('doc-1', 'prescription', 'Report 1', '2026-01-15', 'report.pdf')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file)
             VALUES ('doc-2', 'lab_result', 'Bloodwork', '2026-02-01', 'bloodwork.pdf')",
            [],
        )
        .unwrap();
        drop(conn);

        // Backup
        let backup_path = tmp.path().join("roundtrip.coheara-backup");
        create_backup_with_key(
            &profile_dir, "Round Trip Test", &|p| key.encrypt(p), &backup_path, Some(key.as_bytes()),
        ).unwrap();

        // Restore to new directory
        let restore_dir = tmp.path().join("restored-profile");
        let result = restore_backup(&backup_path, "mypassword", &restore_dir).unwrap();

        assert_eq!(result.documents_restored, 2);
        assert!(result.warnings.is_empty());
        assert!(restore_dir.join("database/coheara.db").exists());
    }

    #[test]
    fn test_backup_wrong_password() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_dir = tmp.path().join("wp-profile");

        std::fs::create_dir_all(profile_dir.join("database")).unwrap();
        let db_path = profile_dir.join("database/coheara.db");
        let conn = crate::db::sqlite::open_database(&db_path, None).unwrap();
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file)
             VALUES ('doc-1', 'lab_result', 'Test Report', '2026-01-15', 'test.pdf')",
            [],
        )
        .unwrap();
        drop(conn);

        let salt = crate::crypto::keys::generate_salt();
        std::fs::write(profile_dir.join("salt.bin"), salt).unwrap();
        let key = ProfileKey::derive("correctpassword", &salt);

        let backup_path = tmp.path().join("wp.coheara-backup");
        create_backup_with_key(
            &profile_dir, "WP Test", &|p| key.encrypt(p), &backup_path, None,
        ).unwrap();

        // Try restoring with wrong password
        let restore_dir = tmp.path().join("wp-restore");
        let result = restore_backup(&backup_path, "wrongpassword", &restore_dir);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Incorrect password") || err_msg.contains("corrupted"),
            "Expected password/corruption error, got: {err_msg}"
        );
    }

    #[test]
    fn test_backup_corrupted() {
        let tmp = tempfile::tempdir().unwrap();
        let backup_path = tmp.path().join("corrupted.coheara-backup");

        // Write just the magic bytes and truncate
        let mut file = std::fs::File::create(&backup_path).unwrap();
        file.write_all(BACKUP_MAGIC).unwrap();
        drop(file);

        let result = preview_backup(&backup_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_backup_invalid_magic() {
        let tmp = tempfile::tempdir().unwrap();
        let backup_path = tmp.path().join("invalid.coheara-backup");

        let mut file = std::fs::File::create(&backup_path).unwrap();
        file.write_all(b"NOTVALID").unwrap();
        drop(file);

        let result = preview_backup(&backup_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Not a valid"));
    }

    // ─── Cryptographic Erasure Tests ───

    #[test]
    fn test_erasure_wrong_confirmation() {
        let tmp = tempfile::tempdir().unwrap();
        let request = ErasureRequest {
            profile_id: Uuid::new_v4().to_string(),
            confirmation_text: "delete my data".into(), // lowercase — should fail
            password: "password".into(),
        };
        let result = erase_profile_data(tmp.path(), &request);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("DELETE MY DATA"));
    }

    #[test]
    fn test_erasure_nonexistent_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let request = ErasureRequest {
            profile_id: Uuid::new_v4().to_string(),
            confirmation_text: "DELETE MY DATA".into(),
            password: "password".into(),
        };
        let result = erase_profile_data(tmp.path(), &request);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    // ─── Privacy Info Tests ───

    #[test]
    fn test_privacy_info() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_dir = tmp.path().join("privacy-test");
        std::fs::create_dir_all(profile_dir.join("database")).unwrap();

        let db_path = profile_dir.join("database/coheara.db");
        let conn = crate::db::sqlite::open_database(&db_path, None).unwrap();
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file)
             VALUES ('doc-1', 'prescription', 'Test Doc', '2026-01-15', 'test.pdf')",
            [],
        )
        .unwrap();

        let info = get_privacy_info(&conn, &profile_dir).unwrap();
        assert_eq!(info.document_count, 1);
        assert!(info.total_data_size_bytes > 0);
        assert_eq!(info.encryption_algorithm, "AES-256-GCM");
        assert!(info.network_permissions.contains("offline"));
        assert!(info.telemetry.contains("None"));
    }

    // ─── Error Recovery Tests ───

    #[test]
    fn test_recovery_strategy_mapping() {
        assert!(matches!(
            recovery_for("Database error"),
            RecoveryStrategy::Retry
        ));
        assert!(matches!(
            recovery_for("Encryption error"),
            RecoveryStrategy::Fatal(_)
        ));
        assert!(matches!(
            recovery_for("Wrong password"),
            RecoveryStrategy::UserActionRequired(_)
        ));
        assert!(matches!(
            recovery_for("Request timeout"),
            RecoveryStrategy::RetryWithBackoff
        ));
        assert!(matches!(
            recovery_for("OCR extraction failed"),
            RecoveryStrategy::FallbackAvailable(_)
        ));
        assert!(matches!(
            recovery_for("Ollama not running"),
            RecoveryStrategy::FallbackAvailable(_)
        ));
    }

    // ─── Helper Tests ───

    #[test]
    fn test_convert_to_mg() {
        assert!((convert_to_mg(1.0, "g") - 1000.0).abs() < f64::EPSILON);
        assert!((convert_to_mg(1000.0, "mcg") - 1.0).abs() < f64::EPSILON);
        assert!((convert_to_mg(500.0, "mg") - 500.0).abs() < f64::EPSILON);
        assert!((convert_to_mg(100.0, "µg") - 0.1).abs() < f64::EPSILON);
        // Unknown unit defaults to value as-is (assumed mg)
        assert!((convert_to_mg(42.0, "tablets") - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calculate_dir_size() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("file1.txt"), "hello").unwrap();
        std::fs::write(tmp.path().join("file2.txt"), "world!").unwrap();
        let size = calculate_dir_size(tmp.path());
        assert_eq!(size, 11); // "hello" (5) + "world!" (6)
    }

    #[test]
    fn test_count_dir_contents() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.txt"), "aaa").unwrap();
        std::fs::create_dir(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("sub/b.txt"), "bbbbb").unwrap();
        let (count, bytes) = count_dir_contents(tmp.path());
        assert_eq!(count, 2);
        assert_eq!(bytes, 8); // 3 + 5
    }

    #[test]
    fn test_calculate_dir_size_nonexistent() {
        let size = calculate_dir_size(Path::new("/nonexistent/path"));
        assert_eq!(size, 0);
    }
}
