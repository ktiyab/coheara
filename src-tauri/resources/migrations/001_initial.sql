-- migrations/001_initial.sql
-- Coheara v0.1.0 — Initial schema
-- Source: Tech Spec v1.1 Section 5.2 / L0-02 Data Model spec

PRAGMA journal_mode=DELETE;  -- No WAL for forensic safety
PRAGMA foreign_keys=ON;

-- ═══════════════════════════════════════════
-- PROFESSIONALS (referenced by many tables)
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
-- DOCUMENTS
-- ═══════════════════════════════════════════

CREATE TABLE documents (
    id TEXT PRIMARY KEY NOT NULL,
    type TEXT NOT NULL CHECK (type IN (
        'prescription', 'lab_result', 'clinical_note',
        'discharge_summary', 'radiology_report',
        'pharmacy_record', 'other'
    )),
    title TEXT NOT NULL,
    document_date TEXT,
    ingestion_date TEXT NOT NULL,
    professional_id TEXT REFERENCES professionals(id),
    source_file TEXT NOT NULL,
    markdown_file TEXT,
    ocr_confidence REAL,
    verified INTEGER NOT NULL DEFAULT 0,
    source_deleted INTEGER NOT NULL DEFAULT 0,
    perceptual_hash TEXT,
    notes TEXT
);

CREATE INDEX idx_documents_type ON documents(type);
CREATE INDEX idx_documents_date ON documents(document_date);
CREATE INDEX idx_documents_professional ON documents(professional_id);
CREATE INDEX idx_documents_hash ON documents(perceptual_hash);

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
    condition TEXT,
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
    maps_to_generic TEXT
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
    test_code TEXT,
    value REAL,
    value_text TEXT,
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
    entity_ids TEXT NOT NULL,
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
    source_chunks TEXT,
    confidence REAL,
    feedback TEXT CHECK (feedback IN ('helpful', 'not_helpful'))
);

CREATE INDEX idx_messages_conversation ON messages(conversation_id);
CREATE INDEX idx_messages_timestamp ON messages(timestamp);

-- ═══════════════════════════════════════════
-- PROFILE TRUST METRICS
-- ═══════════════════════════════════════════

CREATE TABLE profile_trust (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    total_documents INTEGER NOT NULL DEFAULT 0,
    documents_verified INTEGER NOT NULL DEFAULT 0,
    documents_corrected INTEGER NOT NULL DEFAULT 0,
    extraction_accuracy REAL NOT NULL DEFAULT 0.0,
    last_updated TEXT NOT NULL
);

INSERT INTO profile_trust (id, total_documents, documents_verified, documents_corrected, extraction_accuracy, last_updated)
VALUES (1, 0, 0, 0, 0.0, datetime('now'));

-- ═══════════════════════════════════════════
-- DOSE REFERENCES (bundled safety data)
-- ═══════════════════════════════════════════

CREATE TABLE dose_references (
    generic_name TEXT PRIMARY KEY NOT NULL,
    typical_min_mg REAL,
    typical_max_mg REAL,
    absolute_max_mg REAL,
    unit TEXT NOT NULL DEFAULT 'mg',
    source TEXT NOT NULL DEFAULT 'bundled'
);

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
