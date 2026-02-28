-- GAP-05: Persist page_count from extraction pipeline to documents table.
ALTER TABLE documents ADD COLUMN page_count INTEGER;

INSERT INTO schema_version (version, applied_at, description)
VALUES (20, datetime('now'), 'GAP-05: page_count on documents table');
