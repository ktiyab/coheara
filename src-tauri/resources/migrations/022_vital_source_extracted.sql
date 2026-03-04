-- Migration 022: Add 'vital_sign' domain + 'extracted' vital source.
-- B4: Batch vital sign extraction from chat.
--
-- SQLite does not support ALTER TABLE ... DROP CONSTRAINT.
-- Recreate tables with updated CHECK constraints.

-- ═══════════════════════════════════════════
-- 1. vital_signs: add 'extracted' to source CHECK
-- ═══════════════════════════════════════════

CREATE TABLE IF NOT EXISTS vital_signs_new (
    id TEXT PRIMARY KEY,
    vital_type TEXT NOT NULL,
    value_primary REAL NOT NULL,
    value_secondary REAL,
    unit TEXT NOT NULL,
    recorded_at TEXT NOT NULL,
    notes TEXT,
    source TEXT NOT NULL DEFAULT 'manual' CHECK(source IN ('manual', 'imported', 'extracted')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO vital_signs_new SELECT * FROM vital_signs;

DROP TABLE vital_signs;

ALTER TABLE vital_signs_new RENAME TO vital_signs;

CREATE INDEX IF NOT EXISTS idx_vital_signs_type_date
    ON vital_signs(vital_type, recorded_at);

-- ═══════════════════════════════════════════
-- 2. extraction_pending: add 'vital_sign' to domain CHECK
--    Includes source_quote column added by migration 015.
-- ═══════════════════════════════════════════

CREATE TABLE IF NOT EXISTS extraction_pending_new (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id),
    batch_id TEXT NOT NULL REFERENCES extraction_batches(id),
    domain TEXT NOT NULL CHECK(domain IN ('symptom', 'medication', 'appointment', 'vital_sign')),
    extracted_data TEXT NOT NULL,
    confidence REAL NOT NULL,
    grounding TEXT NOT NULL CHECK(grounding IN ('grounded', 'partial', 'ungrounded')),
    duplicate_of TEXT,
    source_message_ids TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK(status IN ('pending', 'confirmed', 'edited_confirmed', 'dismissed')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    reviewed_at TEXT,
    source_quote TEXT
);

INSERT INTO extraction_pending_new SELECT * FROM extraction_pending;

DROP TABLE extraction_pending;

ALTER TABLE extraction_pending_new RENAME TO extraction_pending;

CREATE INDEX IF NOT EXISTS idx_extraction_pending_status
    ON extraction_pending(status);

CREATE INDEX IF NOT EXISTS idx_extraction_pending_batch
    ON extraction_pending(batch_id);

CREATE INDEX IF NOT EXISTS idx_extraction_pending_conversation
    ON extraction_pending(conversation_id);

-- Schema version bump
INSERT INTO schema_version (version, applied_at) VALUES (22, datetime('now'));
