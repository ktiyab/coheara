-- Migration 010: Night Batch Extraction tables
-- Source: LP-01 Night Batch Extraction Pipeline
-- These tables live INSIDE per-profile encrypted SQLite databases.

-- ═══════════════════════════════════════════
-- EXTRACTION BATCHES (batch run tracking)
-- ═══════════════════════════════════════════

CREATE TABLE IF NOT EXISTS extraction_batches (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id),
    extracted_at TEXT NOT NULL,           -- ISO 8601: when batch processed this conversation
    domains_found TEXT NOT NULL,          -- JSON array: ["symptom", "medication"]
    items_extracted INTEGER NOT NULL DEFAULT 0,
    model_name TEXT NOT NULL,             -- e.g. "medgemma:4b"
    duration_ms INTEGER,                 -- total extraction time for this conversation
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_extraction_batches_conversation
    ON extraction_batches(conversation_id);

CREATE INDEX IF NOT EXISTS idx_extraction_batches_extracted_at
    ON extraction_batches(extracted_at);

-- ═══════════════════════════════════════════
-- EXTRACTION PENDING (items awaiting user review)
-- ═══════════════════════════════════════════

CREATE TABLE IF NOT EXISTS extraction_pending (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id),
    batch_id TEXT NOT NULL REFERENCES extraction_batches(id),
    domain TEXT NOT NULL CHECK(domain IN ('symptom', 'medication', 'appointment')),
    extracted_data TEXT NOT NULL,         -- JSON: the full extracted item
    confidence REAL NOT NULL,
    grounding TEXT NOT NULL CHECK(grounding IN ('grounded', 'partial', 'ungrounded')),
    duplicate_of TEXT,                   -- existing record ID if duplicate detected
    source_message_ids TEXT NOT NULL,    -- JSON array of message IDs
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK(status IN ('pending', 'confirmed', 'edited_confirmed', 'dismissed')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    reviewed_at TEXT                     -- when user acted on it
);

CREATE INDEX IF NOT EXISTS idx_extraction_pending_status
    ON extraction_pending(status);

CREATE INDEX IF NOT EXISTS idx_extraction_pending_batch
    ON extraction_pending(batch_id);

CREATE INDEX IF NOT EXISTS idx_extraction_pending_conversation
    ON extraction_pending(conversation_id);

-- Schema version bump
INSERT INTO schema_version (version, applied_at) VALUES (10, datetime('now'));
