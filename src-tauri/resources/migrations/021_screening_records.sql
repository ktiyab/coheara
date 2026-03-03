-- 021_screening_records.sql
-- ME-06: User-reported screening and vaccination records.
-- Stores completion dates for screenings and vaccine doses.

CREATE TABLE IF NOT EXISTS screening_records (
    id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL,
    screening_key TEXT NOT NULL,
    dose_number INTEGER NOT NULL DEFAULT 1,
    completed_at TEXT NOT NULL,
    provider TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (profile_id) REFERENCES profiles(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_screening_records_unique
    ON screening_records(profile_id, screening_key, dose_number);

CREATE INDEX IF NOT EXISTS idx_screening_records_profile
    ON screening_records(profile_id);
