-- CT-01: Per-model enabled/disabled flag for pipeline routing.
--
-- Decouples "what can this model do" (tags) from "should the pipeline use it" (enabled).
-- Default is enabled (1). Missing row = enabled (no explicit preference means "use it").
-- Users toggle via Settings > AI Engine to control which models participate in processing.

CREATE TABLE IF NOT EXISTS model_enabled (
    model_name  TEXT PRIMARY KEY,
    enabled     INTEGER NOT NULL DEFAULT 1,
    updated_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO schema_version (version, applied_at) VALUES (18, datetime('now'));
