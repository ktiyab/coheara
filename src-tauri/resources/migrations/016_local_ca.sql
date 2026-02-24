-- migrations/016_local_ca.sql
-- SEC-HTTPS-01: Local CA for HTTPS on local networks.
-- Stores the CA certificate (public, unencrypted) and private key
-- (AES-256-GCM encrypted with profile key). Singleton table (id=1).

CREATE TABLE IF NOT EXISTS local_ca (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    cert_der BLOB NOT NULL,
    key_encrypted BLOB NOT NULL,
    fingerprint TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO schema_version (version, applied_at, description)
VALUES (16, datetime('now'), 'SEC-HTTPS-01: Local CA for HTTPS');
