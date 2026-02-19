-- Migration 009: Grounded Implementation tables
-- Source: TASK-5, Specs 44-47 (AI Pipeline, Onboarding, Caregiver, Features)
-- These tables live INSIDE per-profile encrypted SQLite databases.

-- Vital signs tracking (Spec 47: Feature Enhancements)
CREATE TABLE IF NOT EXISTS vital_signs (
    id TEXT PRIMARY KEY,
    vital_type TEXT NOT NULL CHECK(vital_type IN (
        'temperature', 'blood_pressure', 'weight', 'height',
        'heart_rate', 'blood_glucose', 'oxygen_saturation'
    )),
    value_primary REAL NOT NULL,
    value_secondary REAL,           -- diastolic for blood_pressure
    unit TEXT NOT NULL,
    recorded_at TEXT NOT NULL,
    notes TEXT,
    source TEXT NOT NULL DEFAULT 'manual' CHECK(source IN ('manual', 'imported')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_vital_signs_type_date
    ON vital_signs(vital_type, recorded_at);

-- Cached AI explanations — pre-computed at import time (Spec 44: AI Pipeline)
CREATE TABLE IF NOT EXISTS cached_explanations (
    id TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL CHECK(entity_type IN (
        'lab_result', 'medication', 'diagnosis', 'document'
    )),
    entity_id TEXT NOT NULL,
    explanation_text TEXT NOT NULL,
    language TEXT NOT NULL DEFAULT 'en',
    model_version TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    invalidated_at TEXT,
    UNIQUE(entity_type, entity_id, language)
);

CREATE INDEX IF NOT EXISTS idx_cached_explanations_entity
    ON cached_explanations(entity_type, entity_id);

-- FTS5 document search index (Spec 46: Caregiver — document search)
-- Populated via triggers from the documents table.
CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts USING fts5(
    title,
    professional_name,
    content_summary,
    content='',                     -- external content mode (we manage sync ourselves)
    tokenize='unicode61 remove_diacritics 2'
);

-- Schema version bump
INSERT INTO schema_version (version, applied_at) VALUES (9, datetime('now'));
