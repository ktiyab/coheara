-- migrations/007_model_preferences.sql
-- L6-04: Model Preferences — per-profile model selection and generic preferences.
-- Source: L6-04-MODEL-PREFERENCES.md

PRAGMA foreign_keys=ON;

-- ═══════════════════════════════════════════
-- MODEL PREFERENCES TABLE (singleton)
-- ═══════════════════════════════════════════
-- Stores the user's active AI model selection per profile.
-- Singleton pattern: id=1 CHECK constraint, pre-seeded row.
-- SEC-L6-13: Model name validated in Rust before write.

CREATE TABLE IF NOT EXISTS model_preferences (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    active_model    TEXT,
    model_quality   TEXT DEFAULT 'unknown',
    set_at          DATETIME,
    set_by          TEXT DEFAULT 'user',
    CONSTRAINT valid_quality CHECK (model_quality IN ('medical', 'general', 'unknown')),
    CONSTRAINT valid_source CHECK (set_by IN ('user', 'wizard', 'fallback'))
);

INSERT OR IGNORE INTO model_preferences (id, active_model, model_quality, set_at, set_by)
VALUES (1, NULL, 'unknown', NULL, 'user');

-- ═══════════════════════════════════════════
-- USER PREFERENCES TABLE (generic key-value)
-- ═══════════════════════════════════════════
-- Stores generic per-profile preferences (dismissed_ai_setup, theme, etc.).
-- SEC-L6-16: Keys whitelisted in Rust code.

CREATE TABLE IF NOT EXISTS user_preferences (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Schema version
INSERT INTO schema_version (version, applied_at, description)
VALUES (7, datetime('now'), 'L6-04: Model preferences and user preferences');
