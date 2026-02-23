-- Migration 015: Add source_quote to extraction_pending
-- LP-01 GAP 9: Source conversation excerpt for ReviewCards (REV-12).

ALTER TABLE extraction_pending ADD COLUMN source_quote TEXT;

-- Schema version bump
INSERT INTO schema_version (version, applied_at) VALUES (15, datetime('now'));
