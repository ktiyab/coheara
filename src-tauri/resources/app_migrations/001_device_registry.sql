-- 001_device_registry.sql
-- App-level database schema for global device registry.
-- Stored unencrypted at profiles_dir/app.db — analogous to Android's system database.
-- Persists device pairings across profile switches.

CREATE TABLE IF NOT EXISTS schema_version (
    version    INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO schema_version (version) VALUES (1);

-- Global device registry: devices persist across profile switches.
-- owner_profile_id identifies which profile originally paired this device.
CREATE TABLE device_registry (
    device_id        TEXT PRIMARY KEY,
    device_name      TEXT NOT NULL,
    device_model     TEXT NOT NULL,
    owner_profile_id TEXT NOT NULL,
    public_key       BLOB NOT NULL,
    paired_at        TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen        TEXT NOT NULL DEFAULT (datetime('now')),
    is_revoked       INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_device_registry_owner ON device_registry(owner_profile_id);

-- Which profiles each device can access (Google Family Link pattern).
-- Automatically populated: owner's profile + all managed_by profiles on pairing.
CREATE TABLE device_profile_access (
    device_id    TEXT NOT NULL REFERENCES device_registry(device_id) ON DELETE CASCADE,
    profile_id   TEXT NOT NULL,
    access_level TEXT NOT NULL CHECK (access_level IN ('full', 'read_only')) DEFAULT 'full',
    granted_at   TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (device_id, profile_id)
);

CREATE INDEX idx_device_profile_access_profile ON device_profile_access(profile_id);

-- Cross-profile access grants (user-to-user, 1Password Families pattern).
-- Unidirectional: Alice grants Bob != Bob grants Alice.
CREATE TABLE profile_access_grants (
    id                  TEXT PRIMARY KEY,
    granter_profile_id  TEXT NOT NULL,
    grantee_profile_id  TEXT NOT NULL,
    access_level        TEXT NOT NULL CHECK (access_level IN ('full', 'read_only')) DEFAULT 'read_only',
    granted_at          TEXT NOT NULL DEFAULT (datetime('now')),
    revoked_at          TEXT,
    UNIQUE(granter_profile_id, grantee_profile_id)
);

CREATE INDEX idx_profile_access_grants_grantee ON profile_access_grants(grantee_profile_id);
CREATE INDEX idx_profile_access_grants_granter ON profile_access_grants(granter_profile_id);
