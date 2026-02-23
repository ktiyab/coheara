-- migrations/014_audit_profile.sql
-- E8: Audit trail enrichment — track which profile's data was accessed.
-- "Device X accessed Profile Y's data at time Z" — medical compliance.

ALTER TABLE audit_log ADD COLUMN profile_id TEXT;
CREATE INDEX idx_audit_profile ON audit_log(profile_id);

-- Schema version
INSERT INTO schema_version (version, applied_at, description)
VALUES (14, datetime('now'), 'E8: Audit trail profile_id column');
