-- migrations/005_audit_log.sql
-- RS-ME-01-001: Persist audit events to SQLite (was in-memory only)
-- Source: ME-01-API-ABSTRACTION.md, E2E review finding

PRAGMA foreign_keys=ON;

-- ═══════════════════════════════════════════
-- AUDIT LOG TABLE
-- ═══════════════════════════════════════════
-- Persists access events for compliance and security audit trail.
-- 90-day retention; pruned on profile unlock.

CREATE TABLE IF NOT EXISTS audit_log (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    source    TEXT NOT NULL,    -- 'desktop' or 'mobile:<device_id>'
    action    TEXT NOT NULL,
    entity    TEXT NOT NULL
);

CREATE INDEX idx_audit_timestamp ON audit_log(timestamp);
CREATE INDEX idx_audit_source ON audit_log(source);

-- Schema version
INSERT INTO schema_version (version, applied_at, description)
VALUES (5, datetime('now'), 'RS-ME-01-001: Audit log persistence');
