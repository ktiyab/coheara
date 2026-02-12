# L0-02 — Data Model

<!--
=============================================================================
COMPONENT SPEC — The entire structured data layer.
Engineer review: E-DA (Data), E-RS (Rust), E-SC (Security), E-QA (QA)
This is the MOST critical foundation component.
Every other component reads or writes through these structures.
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=22 limit=18` |
| [2] Dependencies | `offset=40 limit=12` |
| [3] Interfaces (Rust Traits) | `offset=52 limit=80` |
| [4] SQLite Schema (Full DDL) | `offset=132 limit=250` |
| [5] LanceDB Schema | `offset=382 limit=40` |
| [6] Rust Model Structs | `offset=422 limit=200` |
| [7] Migration System | `offset=622 limit=40` |
| [8] Error Handling | `offset=662 limit=25` |
| [9] Security | `offset=687 limit=20` |
| [10] Testing | `offset=707 limit=50` |
| [11] Performance | `offset=757 limit=20` |
| [12] Open Questions | `offset=777 limit=15` |

---

## [1] Identity

**What:** Implement the complete dual-layer data model: SQLite (18 structured tables) + LanceDB (2 vector tables). Define all Rust structs, enums, and repository traits. Set up migration system.

**After this session:**
- SQLite database initializes with all 18 tables
- LanceDB vector store initializes with 2 tables
- All Rust model structs compile and serialize/deserialize correctly
- Repository traits defined for CRUD operations
- Basic SQLite repository implementations for core tables
- Migration system runs on first launch
- All unit tests pass

**Estimated complexity:** High
**Source:** Tech Spec v1.1 Section 5 (Data Model)

---

## [2] Dependencies

**Incoming:** L0-01 (project scaffold must exist)

**Outgoing:** L0-03 (encryption wraps this), L1-04 (storage pipeline writes here), L2-01 (RAG reads here), L2-03 (coherence reads here), all L3/L4 screens read here.

**New Cargo.toml dependencies:**
```toml
rusqlite = { version = "0.32", features = ["bundled", "chrono", "uuid"] }
lancedb = "0.13"
arrow = { version = "53", features = ["json"] }
```

---

## [3] Interfaces (Rust Traits)

**E-RS + E-DA joint design:** One repository trait per domain entity. Generic enough to support both real and mock implementations.

### Core Repository Trait

```rust
/// Base trait for all entity repositories
pub trait Repository<T, F> {
    fn insert(&self, entity: &T) -> Result<Uuid, DatabaseError>;
    fn get(&self, id: &Uuid) -> Result<Option<T>, DatabaseError>;
    fn update(&self, entity: &T) -> Result<(), DatabaseError>;
    fn delete(&self, id: &Uuid) -> Result<(), DatabaseError>;
    fn list(&self, filter: &F) -> Result<Vec<T>, DatabaseError>;
}
```

### Domain-Specific Traits

```rust
pub trait DocumentRepository: Repository<Document, DocumentFilter> {
    fn get_by_hash(&self, hash: &str) -> Result<Option<Document>, DatabaseError>;
    fn mark_source_deleted(&self, id: &Uuid) -> Result<(), DatabaseError>;
}

pub trait MedicationRepository: Repository<Medication, MedicationFilter> {
    fn get_active(&self) -> Result<Vec<Medication>, DatabaseError>;
    fn get_by_generic_name(&self, name: &str) -> Result<Vec<Medication>, DatabaseError>;
    fn add_dose_change(&self, change: &DoseChange) -> Result<(), DatabaseError>;
    fn get_dose_history(&self, med_id: &Uuid) -> Result<Vec<DoseChange>, DatabaseError>;
    fn get_compound_ingredients(&self, med_id: &Uuid) -> Result<Vec<CompoundIngredient>, DatabaseError>;
    fn get_tapering_schedule(&self, med_id: &Uuid) -> Result<Vec<TaperingStep>, DatabaseError>;
}

pub trait LabResultRepository: Repository<LabResult, LabResultFilter> {
    fn get_by_test_name(&self, name: &str) -> Result<Vec<LabResult>, DatabaseError>;
    fn get_critical(&self) -> Result<Vec<LabResult>, DatabaseError>;
    fn get_trending(&self, test_name: &str, limit: usize) -> Result<Vec<LabResult>, DatabaseError>;
}

pub trait AllergyRepository: Repository<Allergy, AllergyFilter> {
    fn get_all_active(&self) -> Result<Vec<Allergy>, DatabaseError>;
    fn check_against_medication(&self, generic_name: &str) -> Result<Vec<Allergy>, DatabaseError>;
}

pub trait SymptomRepository: Repository<Symptom, SymptomFilter> {
    fn get_active(&self) -> Result<Vec<Symptom>, DatabaseError>;
    fn get_in_date_range(&self, start: NaiveDate, end: NaiveDate) -> Result<Vec<Symptom>, DatabaseError>;
    fn get_by_medication(&self, med_id: &Uuid) -> Result<Vec<Symptom>, DatabaseError>;
}

pub trait ProfessionalRepository: Repository<Professional, ProfessionalFilter> {
    fn find_or_create(&self, name: &str, specialty: Option<&str>) -> Result<Professional, DatabaseError>;
}

pub trait ConversationRepository {
    fn create_conversation(&self) -> Result<Uuid, DatabaseError>;
    fn add_message(&self, msg: &Message) -> Result<(), DatabaseError>;
    fn get_messages(&self, conversation_id: &Uuid) -> Result<Vec<Message>, DatabaseError>;
    fn search_messages(&self, query: &str) -> Result<Vec<Message>, DatabaseError>;
}

pub trait AlertRepository {
    fn dismiss(&self, alert: &DismissedAlert) -> Result<(), DatabaseError>;
    fn is_dismissed(&self, alert_type: &str, entity_ids: &[Uuid]) -> Result<bool, DatabaseError>;
    fn get_dismissed(&self) -> Result<Vec<DismissedAlert>, DatabaseError>;
}

pub trait ProfileTrustRepository {
    fn get_trust(&self) -> Result<ProfileTrust, DatabaseError>;
    fn record_verified(&self) -> Result<(), DatabaseError>;
    fn record_corrected(&self) -> Result<(), DatabaseError>;
}
```

### Vector Store Trait

```rust
pub trait VectorStore {
    fn insert_chunks(&self, chunks: &[DocumentChunk]) -> Result<(), DatabaseError>;
    fn search(&self, query_embedding: &[f32], top_k: usize) -> Result<Vec<SearchResult>, DatabaseError>;
    fn delete_by_document(&self, doc_id: &Uuid) -> Result<(), DatabaseError>;

    fn insert_journal_entry(&self, entry: &JournalEmbedding) -> Result<(), DatabaseError>;
    fn search_journal(&self, query_embedding: &[f32], top_k: usize) -> Result<Vec<SearchResult>, DatabaseError>;
}
```

---

## [4] SQLite Schema (Full DDL)

**E-DA:** Single migration file for v0.1.0. Future schema changes get separate migration files.

```sql
-- migrations/001_initial.sql
-- Coheara v0.1.0 — Initial schema
-- Source: Tech Spec v1.1 Section 5.2

PRAGMA journal_mode=DELETE;  -- E-SC: No WAL for forensic safety
PRAGMA foreign_keys=ON;

-- ═══════════════════════════════════════════
-- DOCUMENTS
-- ═══════════════════════════════════════════

CREATE TABLE documents (
    id TEXT PRIMARY KEY NOT NULL,          -- UUID as TEXT
    type TEXT NOT NULL CHECK (type IN (
        'prescription', 'lab_result', 'clinical_note',
        'discharge_summary', 'radiology_report',
        'pharmacy_record', 'other'
    )),
    title TEXT NOT NULL,
    document_date TEXT,                    -- ISO 8601 date (YYYY-MM-DD)
    ingestion_date TEXT NOT NULL,          -- ISO 8601 datetime
    professional_id TEXT REFERENCES professionals(id),
    source_file TEXT NOT NULL,             -- Path to original
    markdown_file TEXT,                    -- Path to converted .md
    ocr_confidence REAL,                   -- 0.0-1.0
    verified INTEGER NOT NULL DEFAULT 0,   -- Boolean
    source_deleted INTEGER NOT NULL DEFAULT 0,
    perceptual_hash TEXT,                  -- For duplicate detection
    notes TEXT
);

CREATE INDEX idx_documents_type ON documents(type);
CREATE INDEX idx_documents_date ON documents(document_date);
CREATE INDEX idx_documents_professional ON documents(professional_id);
CREATE INDEX idx_documents_hash ON documents(perceptual_hash);

-- ═══════════════════════════════════════════
-- PROFESSIONALS
-- ═══════════════════════════════════════════

CREATE TABLE professionals (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    specialty TEXT,
    institution TEXT,
    first_seen_date TEXT,
    last_seen_date TEXT
);

CREATE INDEX idx_professionals_name ON professionals(name);

-- ═══════════════════════════════════════════
-- MEDICATIONS
-- ═══════════════════════════════════════════

CREATE TABLE medications (
    id TEXT PRIMARY KEY NOT NULL,
    generic_name TEXT NOT NULL,
    brand_name TEXT,
    dose TEXT NOT NULL,
    frequency TEXT NOT NULL,
    frequency_type TEXT NOT NULL CHECK (frequency_type IN (
        'scheduled', 'as_needed', 'tapering'
    )),
    route TEXT NOT NULL DEFAULT 'oral',
    prescriber_id TEXT REFERENCES professionals(id),
    start_date TEXT,
    end_date TEXT,
    reason_start TEXT,
    reason_stop TEXT,
    is_otc INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL CHECK (status IN ('active', 'stopped', 'paused')),
    administration_instructions TEXT,
    max_daily_dose TEXT,
    condition TEXT,                         -- "For pain", "If blood sugar > 250"
    dose_type TEXT NOT NULL DEFAULT 'fixed' CHECK (dose_type IN (
        'fixed', 'sliding_scale', 'weight_based', 'variable'
    )),
    is_compound INTEGER NOT NULL DEFAULT 0,
    document_id TEXT NOT NULL REFERENCES documents(id)
);

CREATE INDEX idx_medications_generic ON medications(generic_name);
CREATE INDEX idx_medications_status ON medications(status);
CREATE INDEX idx_medications_document ON medications(document_id);

-- ═══════════════════════════════════════════
-- MEDICATION EXTENSIONS
-- ═══════════════════════════════════════════

CREATE TABLE compound_ingredients (
    id TEXT PRIMARY KEY NOT NULL,
    medication_id TEXT NOT NULL REFERENCES medications(id) ON DELETE CASCADE,
    ingredient_name TEXT NOT NULL,
    ingredient_dose TEXT,
    maps_to_generic TEXT                   -- For allergy cross-ref
);

CREATE INDEX idx_compound_medication ON compound_ingredients(medication_id);
CREATE INDEX idx_compound_generic ON compound_ingredients(maps_to_generic);

CREATE TABLE tapering_schedules (
    id TEXT PRIMARY KEY NOT NULL,
    medication_id TEXT NOT NULL REFERENCES medications(id) ON DELETE CASCADE,
    step_number INTEGER NOT NULL,
    dose TEXT NOT NULL,
    duration_days INTEGER NOT NULL,
    start_date TEXT,
    document_id TEXT REFERENCES documents(id)
);

CREATE INDEX idx_tapering_medication ON tapering_schedules(medication_id);

CREATE TABLE medication_instructions (
    id TEXT PRIMARY KEY NOT NULL,
    medication_id TEXT NOT NULL REFERENCES medications(id) ON DELETE CASCADE,
    instruction TEXT NOT NULL,
    timing TEXT,
    source_document_id TEXT REFERENCES documents(id)
);

CREATE INDEX idx_instructions_medication ON medication_instructions(medication_id);

CREATE TABLE dose_changes (
    id TEXT PRIMARY KEY NOT NULL,
    medication_id TEXT NOT NULL REFERENCES medications(id) ON DELETE CASCADE,
    old_dose TEXT,
    new_dose TEXT NOT NULL,
    old_frequency TEXT,
    new_frequency TEXT,
    change_date TEXT NOT NULL,
    changed_by_id TEXT REFERENCES professionals(id),
    reason TEXT,
    document_id TEXT REFERENCES documents(id)
);

CREATE INDEX idx_dose_changes_medication ON dose_changes(medication_id);
CREATE INDEX idx_dose_changes_date ON dose_changes(change_date);

-- ═══════════════════════════════════════════
-- LAB RESULTS
-- ═══════════════════════════════════════════

CREATE TABLE lab_results (
    id TEXT PRIMARY KEY NOT NULL,
    test_name TEXT NOT NULL,
    test_code TEXT,                         -- LOINC if extractable
    value REAL,
    value_text TEXT,                        -- For non-numeric
    unit TEXT,
    reference_range_low REAL,
    reference_range_high REAL,
    abnormal_flag TEXT NOT NULL DEFAULT 'normal' CHECK (abnormal_flag IN (
        'normal', 'low', 'high', 'critical_low', 'critical_high'
    )),
    collection_date TEXT NOT NULL,
    lab_facility TEXT,
    ordering_physician_id TEXT REFERENCES professionals(id),
    document_id TEXT NOT NULL REFERENCES documents(id)
);

CREATE INDEX idx_labs_test_name ON lab_results(test_name);
CREATE INDEX idx_labs_date ON lab_results(collection_date);
CREATE INDEX idx_labs_abnormal ON lab_results(abnormal_flag);
CREATE INDEX idx_labs_document ON lab_results(document_id);

-- ═══════════════════════════════════════════
-- DIAGNOSES
-- ═══════════════════════════════════════════

CREATE TABLE diagnoses (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    icd_code TEXT,
    date_diagnosed TEXT,
    diagnosing_professional_id TEXT REFERENCES professionals(id),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN (
        'active', 'resolved', 'monitoring'
    )),
    document_id TEXT NOT NULL REFERENCES documents(id)
);

CREATE INDEX idx_diagnoses_status ON diagnoses(status);

-- ═══════════════════════════════════════════
-- ALLERGIES (CRITICAL SAFETY TABLE)
-- ═══════════════════════════════════════════

CREATE TABLE allergies (
    id TEXT PRIMARY KEY NOT NULL,
    allergen TEXT NOT NULL,
    reaction TEXT,
    severity TEXT NOT NULL CHECK (severity IN (
        'mild', 'moderate', 'severe', 'life_threatening'
    )),
    date_identified TEXT,
    source TEXT NOT NULL CHECK (source IN (
        'document_extracted', 'patient_reported'
    )),
    document_id TEXT REFERENCES documents(id),
    verified INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_allergies_allergen ON allergies(allergen);

-- ═══════════════════════════════════════════
-- PROCEDURES
-- ═══════════════════════════════════════════

CREATE TABLE procedures (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    date TEXT,
    performing_professional_id TEXT REFERENCES professionals(id),
    facility TEXT,
    outcome TEXT,
    follow_up_required INTEGER NOT NULL DEFAULT 0,
    follow_up_date TEXT,
    document_id TEXT NOT NULL REFERENCES documents(id)
);

CREATE INDEX idx_procedures_date ON procedures(date);

-- ═══════════════════════════════════════════
-- SYMPTOMS (OLDCARTS)
-- ═══════════════════════════════════════════

CREATE TABLE symptoms (
    id TEXT PRIMARY KEY NOT NULL,
    category TEXT NOT NULL,
    specific TEXT NOT NULL,
    severity INTEGER NOT NULL CHECK (severity BETWEEN 1 AND 5),
    body_region TEXT,
    duration TEXT,
    character TEXT,
    aggravating TEXT,
    relieving TEXT,
    timing_pattern TEXT,
    onset_date TEXT NOT NULL,
    onset_time TEXT,
    recorded_date TEXT NOT NULL,
    still_active INTEGER NOT NULL DEFAULT 1,
    resolved_date TEXT,
    related_medication_id TEXT REFERENCES medications(id),
    related_diagnosis_id TEXT REFERENCES diagnoses(id),
    source TEXT NOT NULL CHECK (source IN (
        'patient_reported', 'guided_checkin', 'free_text'
    )),
    notes TEXT
);

CREATE INDEX idx_symptoms_onset ON symptoms(onset_date);
CREATE INDEX idx_symptoms_active ON symptoms(still_active);
CREATE INDEX idx_symptoms_medication ON symptoms(related_medication_id);

-- ═══════════════════════════════════════════
-- APPOINTMENTS
-- ═══════════════════════════════════════════

CREATE TABLE appointments (
    id TEXT PRIMARY KEY NOT NULL,
    professional_id TEXT NOT NULL REFERENCES professionals(id),
    date TEXT NOT NULL,
    type TEXT NOT NULL CHECK (type IN ('upcoming', 'completed')),
    pre_summary_generated INTEGER NOT NULL DEFAULT 0,
    post_notes TEXT
);

CREATE INDEX idx_appointments_date ON appointments(date);
CREATE INDEX idx_appointments_professional ON appointments(professional_id);

-- ═══════════════════════════════════════════
-- REFERRALS
-- ═══════════════════════════════════════════

CREATE TABLE referrals (
    id TEXT PRIMARY KEY NOT NULL,
    referring_professional_id TEXT NOT NULL REFERENCES professionals(id),
    referred_to_professional_id TEXT NOT NULL REFERENCES professionals(id),
    reason TEXT,
    date TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN (
        'pending', 'scheduled', 'completed', 'cancelled'
    )),
    document_id TEXT REFERENCES documents(id)
);

-- ═══════════════════════════════════════════
-- MEDICATION ALIASES (bundled data)
-- ═══════════════════════════════════════════

CREATE TABLE medication_aliases (
    generic_name TEXT NOT NULL,
    brand_name TEXT NOT NULL,
    country TEXT NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('bundled', 'user_added')),
    PRIMARY KEY (generic_name, brand_name, country)
);

CREATE INDEX idx_aliases_brand ON medication_aliases(brand_name);

-- ═══════════════════════════════════════════
-- ALERTS
-- ═══════════════════════════════════════════

CREATE TABLE dismissed_alerts (
    id TEXT PRIMARY KEY NOT NULL,
    alert_type TEXT NOT NULL CHECK (alert_type IN (
        'conflict', 'gap', 'drift', 'ambiguity',
        'duplicate', 'allergy', 'dose', 'critical', 'temporal'
    )),
    entity_ids TEXT NOT NULL,              -- JSON array of UUIDs
    dismissed_date TEXT NOT NULL,
    reason TEXT,
    dismissed_by TEXT NOT NULL CHECK (dismissed_by IN (
        'patient', 'professional_feedback'
    ))
);

-- ═══════════════════════════════════════════
-- CONVERSATIONS
-- ═══════════════════════════════════════════

CREATE TABLE conversations (
    id TEXT PRIMARY KEY NOT NULL,
    started_at TEXT NOT NULL,
    title TEXT
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('patient', 'coheara')),
    content TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    source_chunks TEXT,                    -- JSON array of chunk IDs
    confidence REAL
);

CREATE INDEX idx_messages_conversation ON messages(conversation_id);
CREATE INDEX idx_messages_timestamp ON messages(timestamp);

-- ═══════════════════════════════════════════
-- PROFILE TRUST METRICS
-- ═══════════════════════════════════════════

CREATE TABLE profile_trust (
    id INTEGER PRIMARY KEY CHECK (id = 1),  -- Singleton
    total_documents INTEGER NOT NULL DEFAULT 0,
    documents_verified INTEGER NOT NULL DEFAULT 0,
    documents_corrected INTEGER NOT NULL DEFAULT 0,
    extraction_accuracy REAL NOT NULL DEFAULT 0.0,
    last_updated TEXT NOT NULL
);

-- Initialize singleton row
INSERT INTO profile_trust (id, total_documents, documents_verified, documents_corrected, extraction_accuracy, last_updated)
VALUES (1, 0, 0, 0, 0.0, datetime('now'));

-- ═══════════════════════════════════════════
-- SCHEMA VERSION
-- ═══════════════════════════════════════════

CREATE TABLE schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL,
    description TEXT
);

INSERT INTO schema_version (version, applied_at, description)
VALUES (1, datetime('now'), 'Initial schema — Coheara v0.1.0');
```

---

## [5] LanceDB Schema

**E-DA + E-ML:** Two vector tables with metadata.

```rust
// Vector dimension: 384 (all-MiniLM-L6-v2 output dimension)
pub const EMBEDDING_DIM: usize = 384;

/// Document chunks — semantic layer
/// Fields stored in LanceDB Arrow schema:
///   id: Utf8
///   document_id: Utf8
///   content: Utf8 (chunk text)
///   vector: FixedSizeList(Float32, 384)
///   chunk_index: Int32
///   doc_type: Utf8
///   doc_date: Utf8
///   professional_name: Utf8

/// Journal embeddings — patient-reported data in semantic space
/// Fields:
///   id: Utf8
///   symptom_id: Utf8
///   content: Utf8
///   vector: FixedSizeList(Float32, 384)
///   date: Utf8
```

**LanceDB initialization:**
```rust
use lancedb::connect;

pub async fn init_vector_store(profile_path: &Path) -> Result<VectorDb, DatabaseError> {
    let db_path = profile_path.join("vectors");
    let db = connect(db_path.to_str().unwrap()).execute().await?;

    // Create tables if they don't exist
    // LanceDB creates tables on first insert — we'll define schema at insert time
    Ok(VectorDb { db })
}
```

---

## [6] Rust Model Structs

**E-RS:** Structs match schema 1:1. Derive serde for IPC. Derive Clone for Tauri command returns.

```rust
// models/document.rs
use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: Uuid,
    pub doc_type: DocumentType,
    pub title: String,
    pub document_date: Option<NaiveDate>,
    pub ingestion_date: NaiveDateTime,
    pub professional_id: Option<Uuid>,
    pub source_file: String,
    pub markdown_file: Option<String>,
    pub ocr_confidence: Option<f32>,
    pub verified: bool,
    pub source_deleted: bool,
    pub perceptual_hash: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentType {
    Prescription,
    LabResult,
    ClinicalNote,
    DischargeSummary,
    RadiologyReport,
    PharmacyRecord,
    Other,
}

impl DocumentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Prescription => "prescription",
            Self::LabResult => "lab_result",
            Self::ClinicalNote => "clinical_note",
            Self::DischargeSummary => "discharge_summary",
            Self::RadiologyReport => "radiology_report",
            Self::PharmacyRecord => "pharmacy_record",
            Self::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, DatabaseError> {
        match s {
            "prescription" => Ok(Self::Prescription),
            "lab_result" => Ok(Self::LabResult),
            "clinical_note" => Ok(Self::ClinicalNote),
            "discharge_summary" => Ok(Self::DischargeSummary),
            "radiology_report" => Ok(Self::RadiologyReport),
            "pharmacy_record" => Ok(Self::PharmacyRecord),
            "other" => Ok(Self::Other),
            _ => Err(DatabaseError::InvalidEnum {
                field: "document_type".into(),
                value: s.into(),
            }),
        }
    }
}
```

**E-RS note:** Apply the same enum pattern (with `as_str` / `from_str`) to ALL enums: `FrequencyType`, `MedicationStatus`, `DoseType`, `AbnormalFlag`, `AllergySeverity`, `AllergySource`, `SymptomSource`, `AppointmentType`, `ReferralStatus`, `AlertType`, `DismissedBy`, `DiagnosisStatus`, `MessageRole`.

**Remaining model structs follow the same pattern.** Each struct maps 1:1 to its SQLite table. Each enum has `as_str`/`from_str`. Full implementations for ALL 18 tables' corresponding structs in `models/` submodules.

### Filter Structs

```rust
// Used by Repository::list() to filter queries
#[derive(Debug, Default)]
pub struct DocumentFilter {
    pub doc_type: Option<DocumentType>,
    pub professional_id: Option<Uuid>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub verified_only: bool,
}

#[derive(Debug, Default)]
pub struct MedicationFilter {
    pub status: Option<MedicationStatus>,
    pub generic_name: Option<String>,
    pub prescriber_id: Option<Uuid>,
    pub include_otc: bool,
}

#[derive(Debug, Default)]
pub struct LabResultFilter {
    pub test_name: Option<String>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub abnormal_only: bool,
    pub critical_only: bool,
}

// ... similar for SymptomFilter, AllergyFilter, ProfessionalFilter
```

---

## [7] Migration System

**E-DA:** Simple, version-based migration.

```rust
// db/sqlite.rs
pub fn run_migrations(conn: &Connection) -> Result<(), DatabaseError> {
    let current_version = get_current_version(conn)?;

    let migrations = vec![
        (1, include_str!("../../resources/migrations/001_initial.sql")),
        // Future migrations added here:
        // (2, include_str!("../../resources/migrations/002_add_field.sql")),
    ];

    for (version, sql) in migrations {
        if version > current_version {
            tracing::info!("Running migration v{version}");
            conn.execute_batch(sql)?;
        }
    }

    Ok(())
}

fn get_current_version(conn: &Connection) -> Result<i64, DatabaseError> {
    // If schema_version table doesn't exist, we're at version 0
    let result = conn.query_row(
        "SELECT MAX(version) FROM schema_version",
        [],
        |row| row.get::<_, i64>(0),
    );
    match result {
        Ok(v) => Ok(v),
        Err(_) => Ok(0), // Table doesn't exist yet
    }
}
```

---

## [8] Error Handling

```rust
// db/mod.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("LanceDB error: {0}")]
    Lance(String),

    #[error("Entity not found: {entity_type} with id {id}")]
    NotFound { entity_type: String, id: String },

    #[error("Invalid enum value for {field}: {value}")]
    InvalidEnum { field: String, value: String },

    #[error("Migration failed at version {version}: {reason}")]
    MigrationFailed { version: i64, reason: String },

    #[error("Constraint violated: {0}")]
    ConstraintViolation(String),
}
```

---

## [9] Security

**E-SC:**

| Concern | Mitigation |
|---------|-----------|
| SQL injection | All queries use parameterized statements (rusqlite `params![]`). NEVER string concatenation. |
| UUID predictability | UUIDv4 (random). No sequential IDs. |
| Journal mode | DELETE mode (not WAL) — no WAL files with unencrypted data fragments. |
| Sensitive data in logs | NEVER log entity content. Only log IDs and operation types. |
| Foreign keys | Enforced via `PRAGMA foreign_keys=ON`. Prevents orphaned records. |

---

## [10] Testing

### Acceptance Criteria

| # | Test | Expected |
|---|------|----------|
| T-01 | Database initializes from scratch | All 18 tables created, schema_version = 1 |
| T-02 | Insert and retrieve a Document | Round-trip preserves all fields |
| T-03 | Insert and retrieve a Medication | Including dose_type, compound flag |
| T-04 | Insert and retrieve a LabResult | Including abnormal_flag enum |
| T-05 | Insert and retrieve an Allergy | Including severity enum |
| T-06 | Insert and retrieve a Symptom | All OLDCARTS fields |
| T-07 | Foreign key constraint works | Insert medication without document → error |
| T-08 | Cascade delete works | Delete medication → compound_ingredients deleted |
| T-09 | Duplicate detection query | Same perceptual_hash returns existing doc |
| T-10 | Medication filter: active only | Returns only status='active' |
| T-11 | Lab results: critical only | Returns only abnormal_flag in ('critical_low', 'critical_high') |
| T-12 | Migration idempotent | Run twice → no error, same schema version |
| T-13 | All enum round-trips | Every enum value serializes and deserializes correctly |
| T-14 | LanceDB vector insert + search | Insert vector, search by cosine similarity, get result |
| T-15 | Profile trust singleton | Update and retrieve accuracy metrics |

### Test Helper

```rust
/// Create a temporary in-memory database for testing
pub fn test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    conn
}
```

---

## [11] Performance

| Metric | Target |
|--------|--------|
| Database initialization | < 100ms |
| Single entity insert | < 5ms |
| Medication list query (100 meds) | < 10ms |
| Lab results trending query (50 results) | < 10ms |
| Full-text search on messages | < 50ms (with FTS5 index — Phase 2) |

**Indexes are defined in schema.** Verify with `EXPLAIN QUERY PLAN` that all application queries use indexes.

---

## [12] Open Questions

| # | Question | Status |
|---|---------|--------|
| OQ-01 | Use SQLCipher for transparent encryption or application-level encryption? | Deferred to L0-03 |
| OQ-02 | LanceDB exact API for schema definition at table creation | Verify against lancedb crate docs during implementation |
| OQ-03 | FTS5 for full-text search on messages — enable in Phase 1 or Phase 2? | Phase 2 per spec |
