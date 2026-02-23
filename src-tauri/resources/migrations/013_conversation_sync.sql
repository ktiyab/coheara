-- migrations/013_conversation_sync.sql
-- E7: Conversation sync version tracking for cross-device continuity.
-- Adds 'conversations' entity type to sync_versions so companion devices
-- can detect when conversations or messages change.

-- Add conversations entity type
INSERT OR IGNORE INTO sync_versions (entity_type, version, updated_at)
VALUES ('conversations', 0, datetime('now'));

-- ═══════════════════════════════════════════
-- CONVERSATION TRIGGERS
-- ═══════════════════════════════════════════

CREATE TRIGGER IF NOT EXISTS sync_conv_insert AFTER INSERT ON conversations
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'conversations';
END;

CREATE TRIGGER IF NOT EXISTS sync_conv_update AFTER UPDATE ON conversations
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'conversations';
END;

CREATE TRIGGER IF NOT EXISTS sync_conv_delete AFTER DELETE ON conversations
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'conversations';
END;

-- Messages also bump conversations version
CREATE TRIGGER IF NOT EXISTS sync_msg_insert AFTER INSERT ON messages
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'conversations';
END;

CREATE TRIGGER IF NOT EXISTS sync_msg_update AFTER UPDATE ON messages
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'conversations';
END;

CREATE TRIGGER IF NOT EXISTS sync_msg_delete AFTER DELETE ON messages
BEGIN
    UPDATE sync_versions SET version = version + 1, updated_at = datetime('now')
    WHERE entity_type = 'conversations';
END;

-- Schema version
INSERT INTO schema_version (version, applied_at, description)
VALUES (13, datetime('now'), 'E7: Conversation sync version triggers');
