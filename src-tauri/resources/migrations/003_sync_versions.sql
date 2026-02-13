-- migrations/003_sync_versions.sql
-- M0-04: Sync Engine — version counters for delta-based sync
-- Source: M0-04-SYNC-ENGINE.md Section [4]

PRAGMA foreign_keys=ON;

-- ═══════════════════════════════════════════
-- SYNC VERSION COUNTERS
-- ═══════════════════════════════════════════
-- Each entity type has a monotonic version counter.
-- Triggers auto-increment on INSERT/UPDATE/DELETE.
-- Phone sends its known versions; desktop returns only changed types.

CREATE TABLE sync_versions (
    entity_type TEXT PRIMARY KEY NOT NULL,
    version     INTEGER NOT NULL DEFAULT 0,
    updated_at  TEXT NOT NULL
);

-- Pre-populate all 6 entity types
INSERT INTO sync_versions (entity_type, version, updated_at) VALUES ('medications', 0, datetime('now'));
INSERT INTO sync_versions (entity_type, version, updated_at) VALUES ('labs', 0, datetime('now'));
INSERT INTO sync_versions (entity_type, version, updated_at) VALUES ('timeline', 0, datetime('now'));
INSERT INTO sync_versions (entity_type, version, updated_at) VALUES ('alerts', 0, datetime('now'));
INSERT INTO sync_versions (entity_type, version, updated_at) VALUES ('appointments', 0, datetime('now'));
INSERT INTO sync_versions (entity_type, version, updated_at) VALUES ('profile', 0, datetime('now'));

-- ═══════════════════════════════════════════
-- MEDICATIONS TRIGGERS
-- ═══════════════════════════════════════════

CREATE TRIGGER IF NOT EXISTS sync_meds_insert AFTER INSERT ON medications
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'medications';
END;

CREATE TRIGGER IF NOT EXISTS sync_meds_update AFTER UPDATE ON medications
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'medications';
END;

CREATE TRIGGER IF NOT EXISTS sync_meds_delete AFTER DELETE ON medications
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'medications';
END;

-- Dose changes also bump medications version
CREATE TRIGGER IF NOT EXISTS sync_dose_change_insert AFTER INSERT ON dose_changes
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'medications';
END;

-- ═══════════════════════════════════════════
-- LAB RESULTS TRIGGERS
-- ═══════════════════════════════════════════

CREATE TRIGGER IF NOT EXISTS sync_labs_insert AFTER INSERT ON lab_results
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'labs';
END;

CREATE TRIGGER IF NOT EXISTS sync_labs_update AFTER UPDATE ON lab_results
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'labs';
END;

CREATE TRIGGER IF NOT EXISTS sync_labs_delete AFTER DELETE ON lab_results
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'labs';
END;

-- ═══════════════════════════════════════════
-- TIMELINE TRIGGERS (symptoms = journal entries)
-- ═══════════════════════════════════════════

CREATE TRIGGER IF NOT EXISTS sync_timeline_insert AFTER INSERT ON symptoms
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'timeline';
END;

CREATE TRIGGER IF NOT EXISTS sync_timeline_update AFTER UPDATE ON symptoms
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'timeline';
END;

CREATE TRIGGER IF NOT EXISTS sync_timeline_delete AFTER DELETE ON symptoms
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'timeline';
END;

-- ═══════════════════════════════════════════
-- ALERTS TRIGGERS
-- ═══════════════════════════════════════════

CREATE TRIGGER IF NOT EXISTS sync_alerts_insert AFTER INSERT ON dismissed_alerts
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'alerts';
END;

CREATE TRIGGER IF NOT EXISTS sync_alerts_update AFTER UPDATE ON dismissed_alerts
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'alerts';
END;

CREATE TRIGGER IF NOT EXISTS sync_alerts_delete AFTER DELETE ON dismissed_alerts
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'alerts';
END;

-- ═══════════════════════════════════════════
-- APPOINTMENTS TRIGGERS
-- ═══════════════════════════════════════════

CREATE TRIGGER IF NOT EXISTS sync_appts_insert AFTER INSERT ON appointments
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'appointments';
END;

CREATE TRIGGER IF NOT EXISTS sync_appts_update AFTER UPDATE ON appointments
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'appointments';
END;

CREATE TRIGGER IF NOT EXISTS sync_appts_delete AFTER DELETE ON appointments
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'appointments';
END;

-- ═══════════════════════════════════════════
-- PROFILE TRIGGERS
-- ═══════════════════════════════════════════
-- Profile version bumps on: profile_trust update, allergy changes, document changes

CREATE TRIGGER IF NOT EXISTS sync_profile_trust_update AFTER UPDATE ON profile_trust
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'profile';
END;

CREATE TRIGGER IF NOT EXISTS sync_profile_allergy_insert AFTER INSERT ON allergies
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'profile';
END;

CREATE TRIGGER IF NOT EXISTS sync_profile_allergy_update AFTER UPDATE ON allergies
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'profile';
END;

CREATE TRIGGER IF NOT EXISTS sync_profile_allergy_delete AFTER DELETE ON allergies
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'profile';
END;

-- Schema version
INSERT INTO schema_version (version, applied_at, description)
VALUES (3, datetime('now'), 'M0-04: Sync version counters and triggers');
