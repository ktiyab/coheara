# L5-01 — Trust & Safety

<!--
=============================================================================
COMPONENT SPEC — The hardening layer. Trust is not a feature — it's a promise.
Engineer review: E-SC (Security, lead), E-RS (Rust), E-UX (UI/UX), E-DA (Data), E-QA (QA)
This is the cross-cutting safety layer that ties everything together:
emergency protocol, dose plausibility, backup/restore, cryptographic erasure,
and the privacy verification screen that proves Coheara keeps its promises.
Without this component, Coheara is useful. With it, Coheara is trustworthy.
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=22 limit=40` |
| [2] Dependencies | `offset=62 limit=25` |
| [3] Interfaces | `offset=87 limit=80` |
| [4] Emergency Protocol | `offset=167 limit=75` |
| [5] Dose Plausibility | `offset=242 limit=65` |
| [6] Backup & Restore | `offset=307 limit=80` |
| [7] Cryptographic Erasure | `offset=387 limit=45` |
| [8] Privacy Verification Screen | `offset=432 limit=55` |
| [9] Error Recovery Catalog | `offset=487 limit=60` |
| [10] Tauri Commands (IPC) | `offset=547 limit=75` |
| [11] Svelte Components | `offset=622 limit=110` |
| [12] Error Handling | `offset=732 limit=25` |
| [13] Security | `offset=757 limit=25` |
| [14] Testing | `offset=782 limit=55` |
| [15] Performance | `offset=837 limit=15` |
| [16] Open Questions | `offset=852 limit=10` |

---

## [1] Identity

**What:** The trust and safety layer — cross-cutting hardening that operates across all other components. Five sub-systems:

1. **Emergency Protocol** — Critical lab value detection at ingestion, persistent banner, 2-step dismissal
2. **Dose Plausibility** — Cross-reference extracted doses against known ranges, flag outliers during review
3. **Backup & Restore** — Encrypted `.coheara-backup` archive (SQLite + LanceDB + originals + Markdown), password-protected, verification on restore
4. **Cryptographic Erasure** — Profile deletion that makes all data unrecoverable by deleting the encryption key
5. **Privacy Verification Screen** — Shows where data is stored, data size, last access, and documents the airplane mode test

**After this session:**
- Critical lab values trigger persistent banner with calm but firm wording
- Banner requires 2-step dismissal: "Has your doctor addressed this?" → "Yes, my doctor has seen this result"
- Extracted doses checked against plausibility ranges during review (L3-04 integration)
- Full backup creates single encrypted .coheara-backup file
- Restore from backup with password verification
- Backup verification: shows document count and size after backup
- Profile deletion zeros the key, then deletes encrypted data files
- Privacy screen in Settings shows: data location path, total data size, last backup date, airplane mode test instructions

**Estimated complexity:** High
**Source:** Tech Spec v1.1 Section 8.3 (Emergency Protocol), Section 11.1 (Encryption), Section 11.4 (Encrypted Backups)

---

## [2] Dependencies

**Incoming:**
- L0-03 (encryption — ProfileKey, cryptographic erasure primitives, AES-256-GCM)
- L0-02 (data model — all tables for backup, lab_results for critical values)
- L2-03 (coherence engine — CRITICAL and DOSE detection types)
- L3-04 (review screen — dose plausibility warnings shown during review)
- L3-02 (home screen — emergency banner shown on home)

**Outgoing:**
- All components benefit from backup/restore and erasure
- L3-04 (review screen integrates dose plausibility warnings)
- L3-02 (home screen integrates emergency banner)

**New Cargo.toml dependencies:**
```toml
# Archive
tar = "0.4"         # TAR archive creation
flate2 = "1"        # Gzip compression

# Secure deletion
# (Uses existing: aes-gcm, zeroize from L0-03)
```

---

## [3] Interfaces

### Backend Types

```rust
// src-tauri/src/trust.rs

use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

// ─── Emergency Protocol ───

/// Critical lab alert (from L2-03 coherence engine CRITICAL detection)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalLabAlert {
    pub id: Uuid,
    pub test_name: String,
    pub value: String,
    pub unit: String,
    pub reference_range: String,
    pub abnormal_flag: String,        // "critical_low" or "critical_high"
    pub lab_date: NaiveDate,
    pub lab_facility: String,
    pub document_id: Uuid,
    pub detected_at: NaiveDateTime,
    pub dismissed: bool,
    pub dismissed_at: Option<NaiveDateTime>,
    pub dismissed_reason: Option<String>,
}

/// 2-step dismissal for critical alerts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalDismissRequest {
    pub alert_id: Uuid,
    pub step: DismissStep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DismissStep {
    AskConfirmation,       // Step 1: "Has your doctor addressed this?"
    ConfirmDismissal {      // Step 2: "Yes, my doctor has seen this result"
        reason: String,     // Required confirmation text
    },
}

// ─── Dose Plausibility ───

/// Dose plausibility check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DosePlausibility {
    pub medication_name: String,
    pub extracted_dose: String,
    pub extracted_value: f64,       // Parsed numeric value
    pub extracted_unit: String,
    pub typical_range_low: f64,
    pub typical_range_high: f64,
    pub typical_unit: String,
    pub plausibility: PlausibilityResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlausibilityResult {
    Plausible,
    HighDose { message: String },    // Above typical range
    VeryHighDose { message: String }, // Far above typical range (possible extraction error)
    LowDose { message: String },     // Below typical range
    UnknownMedication,               // Not in plausibility DB
}

/// Plausibility reference entry (bundled DB)
#[derive(Debug, Clone)]
pub struct DoseReference {
    pub generic_name: String,
    pub route: String,           // oral, iv, topical, etc.
    pub min_dose_mg: f64,
    pub max_dose_mg: f64,
    pub max_single_dose_mg: f64,
    pub unit: String,
}

// ─── Backup & Restore ───

/// Backup creation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResult {
    pub backup_path: PathBuf,
    pub total_documents: u32,
    pub total_size_bytes: u64,
    pub created_at: NaiveDateTime,
    pub encrypted: bool,
}

/// Backup metadata (stored in archive header)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub version: u32,              // Backup format version (1)
    pub created_at: NaiveDateTime,
    pub profile_name: String,
    pub document_count: u32,
    pub coheara_version: String,
}

/// Restore result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub documents_restored: u32,
    pub total_size_bytes: u64,
    pub warnings: Vec<String>,
}

/// Restore validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePreview {
    pub metadata: BackupMetadata,
    pub file_count: u32,
    pub total_size_bytes: u64,
    pub compatible: bool,
    pub compatibility_message: Option<String>,
}

// ─── Cryptographic Erasure ───

/// Erasure confirmation (requires explicit intent)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErasureRequest {
    pub profile_id: Uuid,
    pub confirmation_text: String,  // Must be "DELETE MY DATA"
    pub password: String,           // Re-authenticate before erasure
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErasureResult {
    pub profile_name: String,
    pub files_deleted: u32,
    pub bytes_erased: u64,
    pub key_zeroed: bool,
}

// ─── Privacy Verification ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyInfo {
    pub data_location: String,          // Filesystem path
    pub total_data_size_bytes: u64,     // All profile data
    pub document_count: u32,
    pub last_backup_date: Option<NaiveDateTime>,
    pub encryption_algorithm: String,   // "AES-256-GCM"
    pub key_derivation: String,         // "PBKDF2 600K iterations"
    pub network_permissions: String,    // "None (fully offline)"
    pub telemetry: String,              // "None — no tracking, no analytics"
}
```

### Frontend Types

```typescript
// src/lib/types/trust.ts

export interface CriticalLabAlert {
  id: string;
  test_name: string;
  value: string;
  unit: string;
  reference_range: string;
  abnormal_flag: string;
  lab_date: string;
  lab_facility: string;
  document_id: string;
  detected_at: string;
  dismissed: boolean;
}

export interface DosePlausibility {
  medication_name: string;
  extracted_dose: string;
  typical_range_low: number;
  typical_range_high: number;
  plausibility: 'Plausible' | { HighDose: { message: string } } |
                { VeryHighDose: { message: string } } |
                { LowDose: { message: string } } | 'UnknownMedication';
}

export interface BackupResult {
  backup_path: string;
  total_documents: number;
  total_size_bytes: number;
  created_at: string;
}

export interface RestorePreview {
  metadata: { version: number; created_at: string; profile_name: string; document_count: number };
  file_count: number;
  total_size_bytes: number;
  compatible: boolean;
  compatibility_message: string | null;
}

export interface RestoreResult {
  documents_restored: number;
  total_size_bytes: number;
  warnings: string[];
}

export interface ErasureResult {
  profile_name: string;
  files_deleted: number;
  bytes_erased: number;
  key_zeroed: boolean;
}

export interface PrivacyInfo {
  data_location: string;
  total_data_size_bytes: number;
  document_count: number;
  last_backup_date: string | null;
  encryption_algorithm: string;
  key_derivation: string;
  network_permissions: string;
  telemetry: string;
}
```

---

## [4] Emergency Protocol

### Detection (at document ingestion)

When a lab result is stored via L1-04 and the `abnormal_flag` is `critical_low` or `critical_high`, L2-03 Coherence Engine generates a CRITICAL observation. This triggers the emergency protocol.

### Banner Rules

1. **At ingestion review (L3-04):** The critical value is highlighted with amber background. Text: "This result is marked as requiring attention on your lab report."
2. **On Home (L3-02) and Chat (L3-03):** Persistent amber banner: "Your lab report from {date} flags {test} as needing prompt attention. Please contact your doctor or pharmacist soon."
3. **Wording rules:** Use "promptly" / "soon" — **never** "immediately" or "urgently." Calm but not dismissive.
4. **No interpretation:** Do NOT explain what the critical value means clinically.
5. **Appointment prep (L4-02):** Added as PRIORITY item (top of the list).

### 2-Step Dismissal

```rust
/// Handles critical alert dismissal (2-step)
pub fn dismiss_critical_alert(
    conn: &rusqlite::Connection,
    request: &CriticalDismissRequest,
) -> Result<(), CohearaError> {
    match &request.step {
        DismissStep::AskConfirmation => {
            // Step 1: Frontend shows confirmation dialog
            // This is a no-op on backend — just validates the alert exists
            let exists: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM coherence_observations
                 WHERE id = ?1 AND severity = 'critical'",
                params![request.alert_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(CohearaError::NotFound("Critical alert not found".into()));
            }
            Ok(())
        }
        DismissStep::ConfirmDismissal { reason } => {
            // Step 2: Actually dismiss with recorded reason
            if reason.is_empty() {
                return Err(CohearaError::Validation(
                    "Reason required to dismiss critical alert".into()
                ));
            }

            conn.execute(
                "INSERT INTO dismissed_alerts (id, alert_type, entity_ids,
                 dismissed_date, reason, dismissed_by)
                 VALUES (?1, 'critical', '[]', datetime('now'), ?2, 'patient')",
                params![Uuid::new_v4(), reason],
            )?;

            tracing::info!(
                "Critical alert {} dismissed with reason: {}",
                request.alert_id, reason
            );

            Ok(())
        }
    }
}
```

### Dismissal UI Flow

```
Step 1: Patient taps [Dismiss] on critical banner
  → Dialog: "Has your doctor addressed this?"
  → [Yes, my doctor has seen this] [Not yet — keep showing]

Step 2: If "Yes":
  → Dialog: "Please confirm: 'My doctor has addressed this lab result'"
  → [Confirm] [Cancel]
  → On confirm: alert dismissed, stored with reason and timestamp
```

---

## [5] Dose Plausibility

### Plausibility Database

A bundled SQLite table `dose_references` with common medications and their typical dose ranges. Populated from public pharmacological references.

```sql
CREATE TABLE IF NOT EXISTS dose_references (
    generic_name TEXT NOT NULL,
    route TEXT NOT NULL DEFAULT 'oral',
    min_dose_mg REAL NOT NULL,
    max_dose_mg REAL NOT NULL,
    max_single_dose_mg REAL NOT NULL,
    unit TEXT NOT NULL DEFAULT 'mg',
    PRIMARY KEY (generic_name, route)
);

-- Sample data
INSERT INTO dose_references VALUES ('metformin', 'oral', 500, 2550, 1000, 'mg');
INSERT INTO dose_references VALUES ('lisinopril', 'oral', 2.5, 40, 40, 'mg');
INSERT INTO dose_references VALUES ('atorvastatin', 'oral', 10, 80, 80, 'mg');
INSERT INTO dose_references VALUES ('aspirin', 'oral', 75, 325, 1000, 'mg');
INSERT INTO dose_references VALUES ('omeprazole', 'oral', 10, 40, 40, 'mg');
INSERT INTO dose_references VALUES ('amlodipine', 'oral', 2.5, 10, 10, 'mg');
-- ... hundreds more
```

### Plausibility Check

```rust
/// Checks if extracted dose is plausible
pub fn check_dose_plausibility(
    conn: &rusqlite::Connection,
    medication_name: &str,
    dose_value: f64,
    dose_unit: &str,
) -> Result<DosePlausibility, CohearaError> {
    // Normalize medication name (lowercase, trim, resolve alias)
    let generic = resolve_to_generic(conn, medication_name)?;

    // Look up reference
    let reference = conn.query_row(
        "SELECT min_dose_mg, max_dose_mg, max_single_dose_mg, unit
         FROM dose_references WHERE generic_name = ?1 AND route = 'oral'",
        params![generic],
        |row| Ok(DoseReference {
            generic_name: generic.clone(),
            route: "oral".into(),
            min_dose_mg: row.get(0)?,
            max_dose_mg: row.get(1)?,
            max_single_dose_mg: row.get(2)?,
            unit: row.get(3)?,
        }),
    );

    match reference {
        Ok(ref_data) => {
            // Convert to same unit if needed
            let dose_mg = convert_to_mg(dose_value, dose_unit);

            let plausibility = if dose_mg > ref_data.max_single_dose_mg * 5.0 {
                PlausibilityResult::VeryHighDose {
                    message: format!(
                        "I extracted {}mg for {} but the typical maximum is {}mg. This may be an extraction error — please double-check this value.",
                        dose_mg, medication_name, ref_data.max_single_dose_mg
                    ),
                }
            } else if dose_mg > ref_data.max_dose_mg {
                PlausibilityResult::HighDose {
                    message: format!(
                        "I extracted {}mg for {} but the typical range is {}-{}mg. Please verify this value.",
                        dose_mg, medication_name, ref_data.min_dose_mg, ref_data.max_dose_mg
                    ),
                }
            } else if dose_mg < ref_data.min_dose_mg * 0.5 {
                PlausibilityResult::LowDose {
                    message: format!(
                        "I extracted {}mg for {} but the typical minimum is {}mg. Please verify this value.",
                        dose_mg, medication_name, ref_data.min_dose_mg
                    ),
                }
            } else {
                PlausibilityResult::Plausible
            };

            Ok(DosePlausibility {
                medication_name: medication_name.into(),
                extracted_dose: format!("{}{}", dose_value, dose_unit),
                extracted_value: dose_value,
                extracted_unit: dose_unit.into(),
                typical_range_low: ref_data.min_dose_mg,
                typical_range_high: ref_data.max_dose_mg,
                typical_unit: ref_data.unit,
                plausibility,
            })
        }
        Err(_) => Ok(DosePlausibility {
            medication_name: medication_name.into(),
            extracted_dose: format!("{}{}", dose_value, dose_unit),
            extracted_value: dose_value,
            extracted_unit: dose_unit.into(),
            typical_range_low: 0.0,
            typical_range_high: 0.0,
            typical_unit: "mg".into(),
            plausibility: PlausibilityResult::UnknownMedication,
        }),
    }
}

fn convert_to_mg(value: f64, unit: &str) -> f64 {
    match unit.to_lowercase().as_str() {
        "mg" => value,
        "g" => value * 1000.0,
        "mcg" | "ug" | "µg" => value / 1000.0,
        _ => value,  // Assume mg if unknown
    }
}
```

### Integration with L3-04 Review Screen

During review, each extracted medication runs through `check_dose_plausibility`. Non-plausible results are shown as inline warnings below the medication field:

```
Metformin 5000mg 2x daily
⚠ I extracted 5000mg but the typical range is 500-2550mg.
  This may be an extraction error — please double-check this value.
```

---

## [6] Backup & Restore

### Backup Format

A `.coheara-backup` file is a gzip-compressed TAR archive encrypted with AES-256-GCM.

```
.coheara-backup structure:
├── metadata.json       (BackupMetadata — unencrypted header for preview)
└── payload.enc        (AES-256-GCM encrypted tar.gz containing:)
    ├── sqlite.db       (Full SQLite database)
    ├── lancedb/        (LanceDB directory)
    ├── originals/      (Original imported files)
    ├── markdown/       (Structured Markdown files)
    └── exports/        (Generated PDFs)
```

### Backup Creation

```rust
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::Write;

/// Creates an encrypted backup of the entire profile
pub fn create_backup(
    session: &ProfileSession,
    output_path: &Path,
) -> Result<BackupResult, CohearaError> {
    let data_dir = session.profile_data_dir();
    let conn = session.db_connection()?;

    // Count documents for metadata
    let doc_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM documents", [], |row| row.get(0)
    )?;

    let metadata = BackupMetadata {
        version: 1,
        created_at: chrono::Local::now().naive_local(),
        profile_name: session.profile_name().into(),
        document_count: doc_count,
        coheara_version: env!("CARGO_PKG_VERSION").into(),
    };

    // 1. Create TAR archive in memory (or temp file for large backups)
    let mut tar_bytes = Vec::new();
    {
        let gz = GzEncoder::new(&mut tar_bytes, Compression::default());
        let mut tar = tar::Builder::new(gz);

        // Add SQLite database
        let db_path = data_dir.join("coheara.db");
        if db_path.exists() {
            tar.append_path_with_name(&db_path, "sqlite.db")?;
        }

        // Add LanceDB directory
        let lance_dir = data_dir.join("lancedb");
        if lance_dir.exists() {
            tar.append_dir_all("lancedb", &lance_dir)?;
        }

        // Add originals directory
        let originals_dir = data_dir.join("originals");
        if originals_dir.exists() {
            tar.append_dir_all("originals", &originals_dir)?;
        }

        // Add markdown directory
        let markdown_dir = data_dir.join("markdown");
        if markdown_dir.exists() {
            tar.append_dir_all("markdown", &markdown_dir)?;
        }

        tar.finish()?;
    }

    // 2. Encrypt the TAR with profile key
    let encrypted_payload = session.encrypt(&tar_bytes)?;

    // 3. Write backup file: metadata header + encrypted payload
    let metadata_json = serde_json::to_vec(&metadata)?;
    let metadata_len = (metadata_json.len() as u32).to_le_bytes();

    let mut file = std::fs::File::create(output_path)?;
    // Magic bytes: "COHEARA\x01"
    file.write_all(b"COHEARA\x01")?;
    // Metadata length (4 bytes LE)
    file.write_all(&metadata_len)?;
    // Metadata JSON (unencrypted — for preview without password)
    file.write_all(&metadata_json)?;
    // Encrypted payload
    file.write_all(&encrypted_payload)?;

    let total_size = std::fs::metadata(output_path)?.len();

    tracing::info!(
        "Backup created: {} documents, {} bytes",
        doc_count, total_size
    );

    Ok(BackupResult {
        backup_path: output_path.to_path_buf(),
        total_documents: doc_count,
        total_size_bytes: total_size,
        created_at: metadata.created_at,
        encrypted: true,
    })
}
```

### Restore

```rust
/// Previews a backup file (reads metadata without decryption)
pub fn preview_backup(
    backup_path: &Path,
) -> Result<RestorePreview, CohearaError> {
    let mut file = std::fs::File::open(backup_path)?;

    // Read magic bytes
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic)?;
    if &magic != b"COHEARA\x01" {
        return Err(CohearaError::Validation("Not a valid Coheara backup file".into()));
    }

    // Read metadata length
    let mut len_bytes = [0u8; 4];
    file.read_exact(&mut len_bytes)?;
    let metadata_len = u32::from_le_bytes(len_bytes) as usize;

    // Read metadata JSON
    let mut metadata_bytes = vec![0u8; metadata_len];
    file.read_exact(&mut metadata_bytes)?;
    let metadata: BackupMetadata = serde_json::from_slice(&metadata_bytes)?;

    let file_size = std::fs::metadata(backup_path)?.len();

    // Check compatibility
    let compatible = metadata.version <= 1;
    let compat_msg = if !compatible {
        Some("This backup was created by a newer version of Coheara.".into())
    } else {
        None
    };

    Ok(RestorePreview {
        metadata,
        file_count: 0,  // Can't count without decryption
        total_size_bytes: file_size,
        compatible,
        compatibility_message: compat_msg,
    })
}

/// Restores a backup (requires password to derive key)
pub fn restore_backup(
    backup_path: &Path,
    password: &str,
    target_data_dir: &Path,
) -> Result<RestoreResult, CohearaError> {
    // 1. Read backup file
    let mut file = std::fs::File::open(backup_path)?;

    // Skip magic + metadata
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic)?;
    let mut len_bytes = [0u8; 4];
    file.read_exact(&mut len_bytes)?;
    let metadata_len = u32::from_le_bytes(len_bytes) as usize;
    let mut metadata_bytes = vec![0u8; metadata_len];
    file.read_exact(&mut metadata_bytes)?;
    let metadata: BackupMetadata = serde_json::from_slice(&metadata_bytes)?;

    // 2. Read encrypted payload
    let mut encrypted_payload = Vec::new();
    file.read_to_end(&mut encrypted_payload)?;

    // 3. Derive key from password and decrypt
    let key = derive_key_from_password(password)?;
    let tar_gz_bytes = decrypt_with_key(&key, &encrypted_payload)
        .map_err(|_| CohearaError::Auth("Incorrect password or corrupted backup".into()))?;

    // 4. Extract TAR to target directory
    let gz = flate2::read::GzDecoder::new(&tar_gz_bytes[..]);
    let mut tar = tar::Archive::new(gz);

    std::fs::create_dir_all(target_data_dir)?;
    tar.unpack(target_data_dir)?;

    // 5. Verify SQLite database
    let db_path = target_data_dir.join("sqlite.db");
    let conn = rusqlite::Connection::open(&db_path)?;
    let doc_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM documents", [], |row| row.get(0)
    )?;

    let total_size = calculate_dir_size(target_data_dir)?;

    // Zero the decrypted TAR from memory
    // (tar_gz_bytes is dropped here, Vec memory will be reclaimed)

    Ok(RestoreResult {
        documents_restored: doc_count,
        total_size_bytes: total_size,
        warnings: Vec::new(),
    })
}
```

### Backup Verification

After backup, show:
```
"247 documents backed up to /path/to/backup.coheara-backup (1.2 GB)"
```

---

## [7] Cryptographic Erasure

### Erasure Process

Profile deletion makes all data unrecoverable by:
1. Zeroing the derived encryption key in memory
2. Deleting the profile's encrypted data directory
3. Removing the profile entry from the global profile registry

Since all data is encrypted with the profile key (AES-256-GCM), and the key is only ever derived from the password (never stored on disk), deleting the data files means the ciphertext is permanently unrecoverable.

```rust
/// Performs cryptographic erasure of a profile
pub fn erase_profile(
    request: &ErasureRequest,
    global_config_path: &Path,
) -> Result<ErasureResult, CohearaError> {
    // 1. Validate confirmation text
    if request.confirmation_text != "DELETE MY DATA" {
        return Err(CohearaError::Validation(
            "Must type 'DELETE MY DATA' to confirm deletion".into()
        ));
    }

    // 2. Verify password (must be able to derive the key)
    let key = derive_key_from_password(&request.password)?;
    verify_key_for_profile(&key, &request.profile_id)?;

    // 3. Get profile data directory
    let data_dir = get_profile_data_dir(global_config_path, &request.profile_id)?;
    let profile_name = get_profile_name(global_config_path, &request.profile_id)?;

    // 4. Count files and size for reporting
    let (file_count, total_bytes) = count_dir_contents(&data_dir)?;

    // 5. Delete all files in the profile directory
    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir)
            .map_err(|e| CohearaError::Io(format!("Failed to delete profile data: {e}")))?;
    }

    // 6. Remove profile from global registry
    remove_profile_from_registry(global_config_path, &request.profile_id)?;

    // 7. Zero the key in memory
    // (key is a ZeroizeOnDrop type from L0-03 — automatically zeroed here)
    drop(key);

    tracing::info!(
        "Profile '{}' (ID: {}) erased: {} files, {} bytes",
        profile_name, request.profile_id, file_count, total_bytes
    );

    Ok(ErasureResult {
        profile_name,
        files_deleted: file_count,
        bytes_erased: total_bytes,
        key_zeroed: true,
    })
}
```

### Erasure UI Flow

```
Settings → Delete Profile

"This will permanently delete all of {name}'s health data.
 This cannot be undone."

Type "DELETE MY DATA" to confirm:
[____________]

Enter your password:
[____________]

[Delete everything]  [Cancel]

→ On success: "Profile deleted. All data has been erased."
→ Navigate to profile picker
```

---

## [8] Privacy Verification Screen

### Purpose

This screen exists so patients (and their caregivers like David the IT son) can verify that Coheara keeps its privacy promises. It shows concrete, inspectable facts.

### Screen Layout

```
┌────────────────────────────────────────────┐
│ PRIVACY & DATA                             │
│                                            │
│ YOUR DATA                                  │
│ ┌────────────────────────────────────────┐ │
│ │ Location: C:\Users\Marie\Coheara\data  │ │
│ │ Total size: 1.2 GB                     │ │
│ │ Documents: 247                         │ │
│ │ Last backup: January 15, 2026          │ │
│ └────────────────────────────────────────┘ │
│                                            │
│ SECURITY                                   │
│ ┌────────────────────────────────────────┐ │
│ │ Encryption: AES-256-GCM               │ │
│ │ Key derivation: PBKDF2 (600K rounds)  │ │
│ │ Network access: None (fully offline)   │ │
│ │ Tracking: None — no analytics          │ │
│ └────────────────────────────────────────┘ │
│                                            │
│ VERIFY IT YOURSELF                         │
│ ┌────────────────────────────────────────┐ │
│ │ 1. Turn on airplane mode               │ │
│ │ 2. Open Coheara                        │ │
│ │ 3. Everything works exactly the same   │ │
│ │                                        │ │
│ │ This proves no internet is needed.     │ │
│ └────────────────────────────────────────┘ │
│                                            │
│ [Open data folder]  [Create backup]        │
│ [Delete profile]                           │
└────────────────────────────────────────────┘
```

### Fetching Privacy Info

```rust
pub fn get_privacy_info(
    session: &ProfileSession,
) -> Result<PrivacyInfo, CohearaError> {
    let data_dir = session.profile_data_dir();
    let total_size = calculate_dir_size(&data_dir)?;

    let conn = session.db_connection()?;
    let doc_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM documents", [], |row| row.get(0)
    )?;

    // Check last backup date from file timestamps in parent directory
    let last_backup = find_latest_backup(&data_dir.parent().unwrap())?;

    Ok(PrivacyInfo {
        data_location: data_dir.to_string_lossy().into_owned(),
        total_data_size_bytes: total_size,
        document_count: doc_count,
        last_backup_date: last_backup,
        encryption_algorithm: "AES-256-GCM".into(),
        key_derivation: "PBKDF2 with 600,000 iterations".into(),
        network_permissions: "None — Coheara works fully offline".into(),
        telemetry: "None — no analytics, no tracking, no crash reporting".into(),
    })
}
```

---

## [9] Error Recovery Catalog

A structured catalog of error recovery flows for each component. Used by all error handlers.

```rust
/// Error recovery strategies per error type
pub enum RecoveryStrategy {
    Retry,                    // Can retry the same operation
    RetryWithBackoff,         // Retry after delay
    FallbackAvailable(String), // Degraded mode available
    UserActionRequired(String), // User must do something
    Fatal(String),             // Cannot recover — show message
}

/// Maps error types to recovery strategies
pub fn recovery_for(error: &CohearaError) -> RecoveryStrategy {
    match error {
        CohearaError::Database(_) => RecoveryStrategy::Retry,
        CohearaError::Encryption(_) => RecoveryStrategy::Fatal(
            "Encryption error — your profile may need to be restored from backup.".into()
        ),
        CohearaError::OcrExtraction(_) => RecoveryStrategy::FallbackAvailable(
            "OCR failed — you can try a clearer photo or enter information manually.".into()
        ),
        CohearaError::LlmUnavailable => RecoveryStrategy::FallbackAvailable(
            "The AI model isn't running. Start Ollama and try again.".into()
        ),
        CohearaError::LlmTimeout => RecoveryStrategy::RetryWithBackoff,
        CohearaError::Network(_) => RecoveryStrategy::UserActionRequired(
            "Check your local network connection.".into()
        ),
        CohearaError::FileNotFound(_) => RecoveryStrategy::UserActionRequired(
            "File not found — it may have been moved or deleted.".into()
        ),
        CohearaError::Io(_) => RecoveryStrategy::Retry,
        CohearaError::Auth(_) => RecoveryStrategy::UserActionRequired(
            "Authentication failed — please check your password.".into()
        ),
        _ => RecoveryStrategy::Retry,
    }
}
```

---

## [10] Tauri Commands (IPC)

```rust
// src-tauri/src/commands/trust.rs

use tauri::State;
use tauri::api::dialog;

/// Gets critical lab alerts
#[tauri::command]
pub async fn get_critical_alerts(
    state: State<'_, AppState>,
) -> Result<Vec<CriticalLabAlert>, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;
    let conn = session.db_connection()?;
    state.update_activity();

    fetch_critical_alerts(&conn).map_err(|e| e.to_string())
}

/// Dismisses a critical alert (2-step)
#[tauri::command]
pub async fn dismiss_critical(
    state: State<'_, AppState>,
    request: CriticalDismissRequest,
) -> Result<(), String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;
    let conn = session.db_connection()?;
    state.update_activity();

    dismiss_critical_alert(&conn, &request).map_err(|e| e.to_string())
}

/// Checks dose plausibility for a medication
#[tauri::command]
pub async fn check_dose(
    state: State<'_, AppState>,
    medication_name: String,
    dose_value: f64,
    dose_unit: String,
) -> Result<DosePlausibility, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;
    let conn = session.db_connection()?;
    state.update_activity();

    check_dose_plausibility(&conn, &medication_name, dose_value, &dose_unit)
        .map_err(|e| e.to_string())
}

/// Creates a backup
#[tauri::command]
pub async fn create_backup(
    state: State<'_, AppState>,
    output_path: String,
) -> Result<BackupResult, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;
    state.update_activity();

    let path = std::path::PathBuf::from(&output_path);
    create_backup_impl(session, &path).map_err(|e| e.to_string())
}

/// Previews a backup file
#[tauri::command]
pub async fn preview_backup_file(
    backup_path: String,
) -> Result<RestorePreview, String> {
    let path = std::path::PathBuf::from(&backup_path);
    preview_backup(&path).map_err(|e| e.to_string())
}

/// Restores from a backup
#[tauri::command]
pub async fn restore_from_backup(
    backup_path: String,
    password: String,
) -> Result<RestoreResult, String> {
    let path = std::path::PathBuf::from(&backup_path);
    // Target directory determined by profile system
    let target = get_new_profile_data_dir()?;
    restore_backup(&path, &password, &target).map_err(|e| e.to_string())
}

/// Erases a profile (cryptographic erasure)
#[tauri::command]
pub async fn erase_profile_data(
    state: State<'_, AppState>,
    request: ErasureRequest,
) -> Result<ErasureResult, String> {
    // Lock current profile first
    state.lock();

    let config_path = get_global_config_path()?;
    erase_profile(&request, &config_path).map_err(|e| e.to_string())
}

/// Gets privacy information
#[tauri::command]
pub async fn get_privacy_info_cmd(
    state: State<'_, AppState>,
) -> Result<PrivacyInfo, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;
    state.update_activity();

    get_privacy_info(session).map_err(|e| e.to_string())
}

/// Opens the data folder in system file manager
#[tauri::command]
pub async fn open_data_folder(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let data_dir = session.profile_data_dir();
    open::that(&data_dir).map_err(|e| format!("Failed to open folder: {e}"))
}
```

### Frontend API

```typescript
// src/lib/api/trust.ts
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import type {
  CriticalLabAlert, CriticalDismissRequest,
  DosePlausibility, BackupResult, RestorePreview,
  RestoreResult, ErasureRequest, ErasureResult, PrivacyInfo
} from '$lib/types/trust';

export async function getCriticalAlerts(): Promise<CriticalLabAlert[]> {
  return invoke<CriticalLabAlert[]>('get_critical_alerts');
}

export async function dismissCritical(request: CriticalDismissRequest): Promise<void> {
  return invoke('dismiss_critical', { request });
}

export async function checkDose(
  medicationName: string, doseValue: number, doseUnit: string
): Promise<DosePlausibility> {
  return invoke<DosePlausibility>('check_dose', { medicationName, doseValue, doseUnit });
}

export async function createBackup(): Promise<BackupResult> {
  const path = await save({
    defaultPath: `coheara-backup-${new Date().toISOString().split('T')[0]}.coheara-backup`,
    filters: [{ name: 'Coheara Backup', extensions: ['coheara-backup'] }],
  });
  if (!path) throw new Error('Cancelled');
  return invoke<BackupResult>('create_backup', { outputPath: path });
}

export async function previewBackup(path: string): Promise<RestorePreview> {
  return invoke<RestorePreview>('preview_backup_file', { backupPath: path });
}

export async function restoreFromBackup(path: string, password: string): Promise<RestoreResult> {
  return invoke<RestoreResult>('restore_from_backup', { backupPath: path, password });
}

export async function eraseProfile(request: ErasureRequest): Promise<ErasureResult> {
  return invoke<ErasureResult>('erase_profile_data', { request });
}

export async function getPrivacyInfo(): Promise<PrivacyInfo> {
  return invoke<PrivacyInfo>('get_privacy_info_cmd');
}

export async function openDataFolder(): Promise<void> {
  return invoke('open_data_folder');
}
```

---

## [11] Svelte Components

### Settings / Privacy Screen

```svelte
<!-- src/lib/components/settings/PrivacyScreen.svelte -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { getPrivacyInfo, createBackup, openDataFolder } from '$lib/api/trust';
  import type { PrivacyInfo, BackupResult } from '$lib/types/trust';
  import BackupRestoreSection from './BackupRestoreSection.svelte';
  import DeleteProfileSection from './DeleteProfileSection.svelte';

  interface Props {
    profileName: string;
    onNavigate: (screen: string) => void;
  }
  let { profileName, onNavigate }: Props = $props();

  let privacyInfo: PrivacyInfo | null = $state(null);
  let loading = $state(true);

  onMount(async () => {
    try {
      privacyInfo = await getPrivacyInfo();
    } catch (e) {
      console.error('Failed to load privacy info:', e);
    } finally {
      loading = false;
    }
  });

  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  }
</script>

<div class="flex flex-col min-h-screen pb-20 bg-stone-50">
  <header class="px-6 pt-6 pb-4">
    <h1 class="text-2xl font-bold text-stone-800">Privacy & Data</h1>
  </header>

  {#if loading}
    <div class="flex-1 flex items-center justify-center">
      <div class="animate-pulse text-stone-400">Loading...</div>
    </div>
  {:else if privacyInfo}
    <div class="px-6 space-y-4">
      <!-- Your Data -->
      <section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
        <h2 class="text-sm font-medium text-stone-500 mb-3">YOUR DATA</h2>
        <div class="space-y-2 text-sm">
          <div class="flex justify-between">
            <span class="text-stone-600">Location</span>
            <span class="text-stone-800 font-mono text-xs max-w-[200px] truncate"
                  title={privacyInfo.data_location}>
              {privacyInfo.data_location}
            </span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600">Total size</span>
            <span class="text-stone-800">{formatBytes(privacyInfo.total_data_size_bytes)}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600">Documents</span>
            <span class="text-stone-800">{privacyInfo.document_count}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600">Last backup</span>
            <span class="text-stone-800">
              {privacyInfo.last_backup_date
                ? new Date(privacyInfo.last_backup_date).toLocaleDateString()
                : 'Never'}
            </span>
          </div>
        </div>
      </section>

      <!-- Security -->
      <section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
        <h2 class="text-sm font-medium text-stone-500 mb-3">SECURITY</h2>
        <div class="space-y-2 text-sm">
          <div class="flex justify-between">
            <span class="text-stone-600">Encryption</span>
            <span class="text-stone-800">{privacyInfo.encryption_algorithm}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600">Key derivation</span>
            <span class="text-stone-800">{privacyInfo.key_derivation}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600">Network access</span>
            <span class="text-green-700 font-medium">{privacyInfo.network_permissions}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600">Tracking</span>
            <span class="text-green-700 font-medium">{privacyInfo.telemetry}</span>
          </div>
        </div>
      </section>

      <!-- Verify It Yourself -->
      <section class="bg-blue-50 rounded-xl p-5 border border-blue-100">
        <h2 class="text-sm font-medium text-blue-700 mb-3">VERIFY IT YOURSELF</h2>
        <ol class="text-sm text-blue-800 space-y-1 list-decimal list-inside">
          <li>Turn on airplane mode</li>
          <li>Open Coheara</li>
          <li>Everything works exactly the same</li>
        </ol>
        <p class="text-xs text-blue-600 mt-2">This proves no internet connection is needed.</p>
      </section>

      <!-- Actions -->
      <div class="flex gap-3">
        <button
          class="flex-1 px-4 py-3 bg-white border border-stone-200 rounded-xl
                 text-sm font-medium text-stone-700 min-h-[44px]"
          onclick={openDataFolder}
        >
          Open data folder
        </button>
        <button
          class="flex-1 px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
                 text-sm font-medium min-h-[44px]"
          onclick={async () => {
            try {
              const result = await createBackup();
              alert(`Backup created: ${result.total_documents} documents, ${formatBytes(result.total_size_bytes)}`);
            } catch (e) {
              if (e !== 'Cancelled') alert('Backup failed: ' + e);
            }
          }}
        >
          Create backup
        </button>
      </div>

      <!-- Backup & Restore -->
      <BackupRestoreSection />

      <!-- Delete Profile (danger zone) -->
      <DeleteProfileSection
        {profileName}
        onDeleted={() => onNavigate('picker')}
      />
    </div>
  {/if}
</div>
```

### Delete Profile Section

```svelte
<!-- src/lib/components/settings/DeleteProfileSection.svelte -->
<script lang="ts">
  import { eraseProfile } from '$lib/api/trust';

  interface Props {
    profileName: string;
    onDeleted: () => void;
  }
  let { profileName, onDeleted }: Props = $props();

  let showConfirm = $state(false);
  let confirmText = $state('');
  let password = $state('');
  let deleting = $state(false);
  let error: string | null = $state(null);

  async function handleDelete() {
    if (confirmText !== 'DELETE MY DATA') {
      error = 'Please type "DELETE MY DATA" exactly';
      return;
    }
    if (!password) {
      error = 'Password required';
      return;
    }

    deleting = true;
    error = null;
    try {
      await eraseProfile({
        profile_id: '', // Filled by backend from active session
        confirmation_text: confirmText,
        password,
      });
      onDeleted();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      deleting = false;
    }
  }
</script>

<section class="mt-8 border-t border-red-200 pt-6">
  <h2 class="text-sm font-medium text-red-600 mb-2">DANGER ZONE</h2>

  {#if !showConfirm}
    <button
      class="w-full px-4 py-3 bg-white border border-red-200 rounded-xl
             text-sm text-red-600 min-h-[44px]"
      onclick={() => showConfirm = true}
    >
      Delete profile and all data
    </button>
  {:else}
    <div class="bg-red-50 rounded-xl p-5 border border-red-200">
      <p class="text-sm text-red-800 mb-4">
        This will permanently delete all of <strong>{profileName}'s</strong> health data.
        This cannot be undone.
      </p>

      <label class="block text-sm text-red-700 mb-1">
        Type "DELETE MY DATA" to confirm:
      </label>
      <input
        type="text"
        class="w-full px-4 py-3 rounded-lg border border-red-200 text-stone-700
               mb-3 min-h-[44px]"
        bind:value={confirmText}
        placeholder="DELETE MY DATA"
      />

      <label class="block text-sm text-red-700 mb-1">Enter your password:</label>
      <input
        type="password"
        class="w-full px-4 py-3 rounded-lg border border-red-200 text-stone-700
               mb-4 min-h-[44px]"
        bind:value={password}
      />

      {#if error}
        <p class="text-red-600 text-sm mb-3">{error}</p>
      {/if}

      <div class="flex gap-3">
        <button
          class="flex-1 px-4 py-3 bg-red-600 text-white rounded-xl text-sm
                 font-medium min-h-[44px] disabled:opacity-50"
          disabled={deleting || confirmText !== 'DELETE MY DATA' || !password}
          onclick={handleDelete}
        >
          {deleting ? 'Deleting...' : 'Delete everything'}
        </button>
        <button
          class="px-4 py-3 bg-white border border-stone-200 rounded-xl text-sm
                 text-stone-600 min-h-[44px]"
          onclick={() => { showConfirm = false; confirmText = ''; password = ''; error = null; }}
        >
          Cancel
        </button>
      </div>
    </div>
  {/if}
</section>
```

---

## [12] Error Handling

| Error | User Message | Recovery |
|-------|-------------|----------|
| Backup creation fails (disk full) | "Not enough disk space for backup." | User frees space or selects different location |
| Backup creation fails (permissions) | "Couldn't write to that location. Try a different folder." | User selects different path |
| Restore wrong password | "Incorrect password. Please try again." | Retry with correct password |
| Restore corrupted backup | "This backup file appears to be damaged." | User tries different backup file |
| Restore incompatible version | "This backup was created by a newer version of Coheara." | User updates Coheara |
| Erasure wrong password | "Incorrect password." | Retry (prevents accidental deletion) |
| Erasure wrong confirmation | "Please type 'DELETE MY DATA' exactly." | User retypes |
| Privacy info load fails | "Couldn't load privacy information." | Retry |
| Critical alert not found | Silent — alert may have been dismissed already | No action needed |

---

## [13] Security

- Backup metadata (version, date, name, count) is unencrypted — allows preview without password. Contains NO medical content.
- Backup payload encrypted with profile key (AES-256-GCM) — same security as live data
- Erasure requires both password AND explicit confirmation text — prevents accidental deletion
- Dose plausibility DB contains only generic reference data — no patient information
- Privacy screen shows factual information only — no sensitive health data
- Critical alert dismissal creates immutable audit trail (dismissed_alerts table)
- `open_data_folder` opens OS file manager — user can independently verify data location

---

## [14] Testing

### Unit Tests (Rust)

| Test | What |
|------|------|
| `test_critical_alert_fetch` | Returns undismissed critical alerts only |
| `test_critical_dismiss_step1` | AskConfirmation validates alert exists |
| `test_critical_dismiss_step2` | ConfirmDismissal stores dismissal record |
| `test_critical_dismiss_requires_reason` | Empty reason rejected |
| `test_dose_plausible` | 500mg metformin → Plausible |
| `test_dose_high` | 3000mg metformin → HighDose |
| `test_dose_very_high` | 50000mg metformin → VeryHighDose (extraction error) |
| `test_dose_low` | 10mg metformin → LowDose |
| `test_dose_unknown_medication` | "xyzabc123" → UnknownMedication |
| `test_dose_unit_conversion` | 1g → 1000mg conversion |
| `test_backup_create` | Backup file created with correct magic bytes |
| `test_backup_metadata_readable` | Metadata readable without decryption |
| `test_backup_restore_roundtrip` | Create → restore → verify all documents present |
| `test_backup_wrong_password` | Restore with wrong password fails gracefully |
| `test_backup_corrupted` | Restore of truncated file fails gracefully |
| `test_erasure_success` | Profile directory deleted, registry updated |
| `test_erasure_wrong_confirmation` | "delete my data" (lowercase) rejected |
| `test_erasure_wrong_password` | Wrong password rejected |
| `test_privacy_info` | Correct data size and document count |
| `test_recovery_strategy_mapping` | Each error type maps to correct recovery |
| `test_convert_to_mg` | g, mcg, mg conversions correct |

### Integration Tests

| Test | What |
|------|------|
| `test_full_backup_restore_cycle` | Create profile → add documents → backup → erase → restore → verify |
| `test_critical_value_triggers_banner` | Import lab with critical value → banner appears on home |
| `test_dose_warning_in_review` | Import prescription with high dose → warning shown in review |
| `test_erasure_makes_data_unrecoverable` | Erase → attempt to read encrypted files → all fail |

### Frontend Tests

| Test | What |
|------|------|
| `test_privacy_screen_renders` | All privacy info sections displayed |
| `test_backup_button_triggers_save_dialog` | Create backup opens file picker |
| `test_delete_profile_requires_confirmation` | Must type "DELETE MY DATA" |
| `test_delete_profile_requires_password` | Password field required |
| `test_verify_section_visible` | Airplane mode instructions shown |
| `test_critical_dismiss_2step` | Both steps required for dismissal |

---

## [15] Performance

- Backup of 247 documents (~1.2 GB): target < 60 seconds
- Restore: target < 90 seconds (decompression + decryption + file extraction)
- Dose plausibility check: < 5ms (single SQLite lookup)
- Critical alert fetch: < 10ms
- Privacy info: < 100ms (directory size scan is the bottleneck)
- Erasure: < 5 seconds (directory deletion)

---

## [16] Open Questions

- **Q1:** Should backup support a separate backup password (different from profile password)? Current answer: no — use profile password. Simplicity > flexibility.
- **Q2:** Should we support incremental backups? Current answer: no — full backup only. Simpler, more reliable. Incremental can be added later.
- **Q3:** Should the dose plausibility DB be user-extensible? Current answer: read-only bundled. Users can report issues but not modify reference data directly.
