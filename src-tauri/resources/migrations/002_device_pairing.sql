-- M0-02: Device pairing tables
-- Stores paired device metadata, session tokens, and TLS certificate.

CREATE TABLE IF NOT EXISTS paired_devices (
    device_id TEXT PRIMARY KEY,
    device_name TEXT NOT NULL,
    device_model TEXT NOT NULL,
    public_key BLOB NOT NULL,        -- X25519 public key (32 bytes)
    paired_at TEXT NOT NULL,          -- ISO 8601
    last_seen TEXT NOT NULL,          -- ISO 8601
    is_revoked INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS device_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id TEXT NOT NULL REFERENCES paired_devices(device_id),
    token_hash BLOB NOT NULL,         -- SHA-256 of current session token (32 bytes)
    prev_token_hash BLOB,             -- Previous token hash (grace period)
    grace_expires TEXT,               -- ISO 8601, NULL if no grace active
    created_at TEXT NOT NULL,          -- ISO 8601
    expires_at TEXT NOT NULL,          -- ISO 8601
    last_used TEXT NOT NULL            -- ISO 8601
);

CREATE INDEX IF NOT EXISTS idx_sessions_token ON device_sessions(token_hash);
CREATE INDEX IF NOT EXISTS idx_sessions_device ON device_sessions(device_id);

CREATE TABLE IF NOT EXISTS server_tls (
    id INTEGER PRIMARY KEY CHECK (id = 1),   -- Singleton row
    private_key_encrypted BLOB NOT NULL,      -- AES-256-GCM encrypted private key
    certificate_der BLOB NOT NULL,            -- Self-signed DER-encoded certificate
    fingerprint TEXT NOT NULL,                -- SHA-256 hex fingerprint
    created_at TEXT NOT NULL                  -- ISO 8601
);

-- Bump schema version
INSERT INTO schema_version (version, applied_at, description)
VALUES (2, datetime('now'), 'Device pairing — M0-02');
