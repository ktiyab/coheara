-- migrations/006_vector_chunks.sql
-- IMP-004: Persistent vector store for semantic search.
-- Stores document chunks with their embeddings in SQLite.
-- Replaces in-memory-only InMemoryVectorStore.

PRAGMA foreign_keys=ON;

CREATE TABLE IF NOT EXISTS vector_chunks (
    id              TEXT PRIMARY KEY NOT NULL,
    document_id     TEXT NOT NULL,
    chunk_index     INTEGER NOT NULL,
    content         BLOB NOT NULL,          -- plaintext or AES-256-GCM encrypted
    is_encrypted    INTEGER NOT NULL DEFAULT 0,
    embedding       BLOB NOT NULL,          -- f32 array as little-endian bytes
    doc_type        TEXT NOT NULL,
    doc_date        TEXT,
    professional_name TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_vector_chunks_document ON vector_chunks(document_id);
CREATE INDEX IF NOT EXISTS idx_vector_chunks_doc_type ON vector_chunks(doc_type);

INSERT INTO schema_version (version, applied_at, description)
VALUES (6, datetime('now'), 'IMP-004: vector_chunks for persistent semantic search');
