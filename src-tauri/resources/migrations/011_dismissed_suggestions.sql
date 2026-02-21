-- LP-07: Track dismissed extraction suggestions to prevent re-surfacing.

CREATE TABLE IF NOT EXISTS dismissed_suggestions (
    id TEXT PRIMARY KEY NOT NULL,
    suggestion_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    dismissed_date TEXT NOT NULL,
    UNIQUE(suggestion_type, entity_id)
);

INSERT INTO schema_version (version, applied_at)
VALUES (11, datetime('now'));
