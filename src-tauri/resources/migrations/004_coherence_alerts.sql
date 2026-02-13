-- migrations/004_coherence_alerts.sql
-- RS-L2-03-001: Persist coherence alerts to SQLite (was in-memory only)
-- Source: L2-03-COHERENCE-ENGINE.md, E2E review finding

PRAGMA foreign_keys=ON;

-- ═══════════════════════════════════════════
-- COHERENCE ALERTS TABLE
-- ═══════════════════════════════════════════
-- Persists active (non-dismissed) alerts so they survive app restart.
-- Dismissed alerts remain in the existing dismissed_alerts table.

CREATE TABLE coherence_alerts (
    id                  TEXT PRIMARY KEY NOT NULL,
    alert_type          TEXT NOT NULL CHECK (alert_type IN (
        'conflict', 'gap', 'drift', 'ambiguity',
        'duplicate', 'allergy', 'dose', 'critical', 'temporal'
    )),
    severity            TEXT NOT NULL CHECK (severity IN ('info', 'standard', 'critical')),
    entity_ids          TEXT NOT NULL,           -- JSON array of UUIDs
    source_document_ids TEXT NOT NULL,           -- JSON array of UUIDs
    patient_message     TEXT NOT NULL,
    detail_json         TEXT NOT NULL,           -- JSON-serialized AlertDetail
    detected_at         TEXT NOT NULL,
    surfaced            INTEGER NOT NULL DEFAULT 0,
    dismissed           INTEGER NOT NULL DEFAULT 0,
    dismissed_date      TEXT,
    dismiss_reason      TEXT,
    dismissed_by        TEXT CHECK (dismissed_by IS NULL OR dismissed_by IN (
        'patient', 'professional_feedback'
    )),
    two_step_confirmed  INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_coherence_alerts_type ON coherence_alerts(alert_type);
CREATE INDEX idx_coherence_alerts_severity ON coherence_alerts(severity);
CREATE INDEX idx_coherence_alerts_dismissed ON coherence_alerts(dismissed);

-- Bump alerts sync version on coherence_alerts changes
CREATE TRIGGER IF NOT EXISTS sync_coherence_alerts_insert AFTER INSERT ON coherence_alerts
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'alerts';
END;

CREATE TRIGGER IF NOT EXISTS sync_coherence_alerts_update AFTER UPDATE ON coherence_alerts
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'alerts';
END;

CREATE TRIGGER IF NOT EXISTS sync_coherence_alerts_delete AFTER DELETE ON coherence_alerts
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'alerts';
END;

-- Schema version
INSERT INTO schema_version (version, applied_at, description)
VALUES (4, datetime('now'), 'RS-L2-03-001: Coherence alerts persistence');
