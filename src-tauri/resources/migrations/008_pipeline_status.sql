-- migrations/008_pipeline_status.sql
-- Block O.6: Track document processing state for recovery.
-- Enables: stuck document detection, reprocessing, status-based queries.

ALTER TABLE documents ADD COLUMN pipeline_status TEXT DEFAULT 'imported';

-- Backfill: verified documents are 'confirmed', unverified are 'imported'
UPDATE documents SET pipeline_status = 'confirmed' WHERE verified = 1;

CREATE INDEX IF NOT EXISTS idx_documents_pipeline_status ON documents(pipeline_status);

INSERT INTO schema_version (version, applied_at, description)
VALUES (8, datetime('now'), 'Block O.6: pipeline_status column for document state tracking');
