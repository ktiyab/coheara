-- migrations/019_butler_v2.sql
-- BTL-10 C1: Butler V2 — processing log + entity connections.
-- Enables: model-to-document traceability, entity-to-entity connections.

-- Track which model processed each document at each pipeline stage.
CREATE TABLE processing_log (
    id TEXT PRIMARY KEY NOT NULL,
    document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    model_name TEXT NOT NULL,
    model_variant TEXT,
    processing_stage TEXT NOT NULL CHECK (processing_stage IN ('extraction', 'structuring')),
    started_at TEXT NOT NULL,
    completed_at TEXT,
    success INTEGER NOT NULL DEFAULT 0,
    error_message TEXT
);
CREATE INDEX idx_processing_log_document ON processing_log(document_id);
CREATE INDEX idx_processing_log_model ON processing_log(model_name);

-- Semantic connections between extracted entities across documents.
CREATE TABLE entity_connections (
    id TEXT PRIMARY KEY NOT NULL,
    source_type TEXT NOT NULL,
    source_id TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    relationship_type TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 0.0,
    document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_entity_conn_source ON entity_connections(source_type, source_id);
CREATE INDEX idx_entity_conn_target ON entity_connections(target_type, target_id);
CREATE INDEX idx_entity_conn_document ON entity_connections(document_id);

INSERT INTO schema_version (version, applied_at, description)
VALUES (19, datetime('now'), 'BTL-10 C1: processing_log + entity_connections tables');
