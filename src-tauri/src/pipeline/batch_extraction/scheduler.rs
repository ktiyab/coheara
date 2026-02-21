//! M1: Batch Scheduler — determines when extraction runs and which conversations qualify.
//!
//! Eligibility rules (LP-01 Section 3.2):
//! - At least 2 messages (back-and-forth)
//! - Last message > cold_hours ago (conversation is "cold")
//! - Not yet extracted, or new messages since last extraction
//! - Max 20 conversations per batch

use chrono::NaiveDateTime;
use rusqlite::{params, Connection};
use uuid::Uuid;

use super::error::ExtractionError;
use super::traits::BatchScheduler;
use super::types::*;
use crate::db::DatabaseError;

/// SQLite-backed batch scheduler.
pub struct SqliteBatchScheduler;

impl SqliteBatchScheduler {
    pub fn new() -> Self {
        Self
    }
}

impl BatchScheduler for SqliteBatchScheduler {
    fn has_pending_work(&self, conn: &Connection) -> Result<bool, ExtractionError> {
        let config = ExtractionConfig::default();
        let conversations = self.get_eligible_conversations(conn, &config)?;
        Ok(!conversations.is_empty())
    }

    fn get_eligible_conversations(
        &self,
        conn: &Connection,
        config: &ExtractionConfig,
    ) -> Result<Vec<ConversationBatch>, ExtractionError> {
        let cold_hours_clause = format!("-{} hours", config.cold_hours);

        // Find conversations eligible for extraction
        let mut stmt = conn.prepare(
            "SELECT c.id, c.title,
                    MAX(m.timestamp) as last_message_at,
                    COUNT(m.id) as message_count
             FROM conversations c
             JOIN messages m ON m.conversation_id = c.id
             LEFT JOIN extraction_batches eb ON c.id = eb.conversation_id
             GROUP BY c.id
             HAVING message_count >= 2
                AND last_message_at < datetime('now', ?1)
                AND (eb.extracted_at IS NULL
                     OR last_message_at > eb.extracted_at)
             ORDER BY last_message_at DESC
             LIMIT ?2"
        ).map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;

        let rows = stmt.query_map(
            params![cold_hours_clause, config.max_conversations_per_batch],
            |row| {
                Ok(ConversationRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    last_message_at: row.get(2)?,
                    message_count: row.get(3)?,
                })
            },
        ).map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;

        let mut conversations = Vec::new();

        for row in rows {
            let row = row.map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;

            // Load messages for this conversation
            let messages = self.load_messages(conn, &row.id)?;

            let last_message_at = NaiveDateTime::parse_from_str(
                &row.last_message_at,
                "%Y-%m-%d %H:%M:%S",
            ).unwrap_or_default();

            conversations.push(ConversationBatch {
                id: row.id,
                title: row.title,
                messages,
                last_message_at,
                message_count: row.message_count,
            });
        }

        Ok(conversations)
    }

    fn mark_extracted(
        &self,
        conn: &Connection,
        conversation_id: &str,
        batch_id: &str,
        domains_found: &[ExtractionDomain],
        items_count: u32,
        model_name: &str,
        duration_ms: u64,
    ) -> Result<(), ExtractionError> {
        let domains_json: Vec<&str> = domains_found.iter().map(|d| d.as_str()).collect();
        let domains_str = serde_json::to_string(&domains_json)
            .map_err(|e| ExtractionError::JsonParsing(e.to_string()))?;
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        conn.execute(
            "INSERT OR REPLACE INTO extraction_batches
             (id, conversation_id, extracted_at, domains_found, items_extracted, model_name, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                batch_id,
                conversation_id,
                now,
                domains_str,
                items_count,
                model_name,
                duration_ms as i64,
            ],
        ).map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;

        Ok(())
    }
}

impl SqliteBatchScheduler {
    fn load_messages(
        &self,
        conn: &Connection,
        conversation_id: &str,
    ) -> Result<Vec<ConversationMessage>, ExtractionError> {
        let mut stmt = conn.prepare(
            "SELECT id, role, content, timestamp
             FROM messages
             WHERE conversation_id = ?1
             ORDER BY timestamp ASC"
        ).map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;

        let rows = stmt.query_map(params![conversation_id], |row| {
            Ok(MessageRow {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                timestamp: row.get(3)?,
            })
        }).map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;

        let mut messages = Vec::new();
        for (idx, row) in rows.enumerate() {
            let row = row.map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;
            messages.push(ConversationMessage {
                id: row.id,
                index: idx,
                role: row.role,
                content: row.content,
                created_at: NaiveDateTime::parse_from_str(
                    &row.timestamp,
                    "%Y-%m-%d %H:%M:%S",
                ).unwrap_or_default(),
                is_signal: false, // Set by analyzer
            });
        }

        Ok(messages)
    }
}

struct ConversationRow {
    id: String,
    title: Option<String>,
    last_message_at: String,
    message_count: u32,
}

struct MessageRow {
    id: String,
    role: String,
    content: String,
    timestamp: String,
}

/// Generate a new batch ID.
pub fn new_batch_id() -> String {
    Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;

    fn setup_db() -> Connection {
        open_memory_database().expect("Failed to open in-memory DB")
    }

    fn insert_conversation(conn: &Connection, id: &str, msgs: &[(&str, &str, &str)]) {
        conn.execute(
            "INSERT INTO conversations (id, started_at, title) VALUES (?1, ?2, ?3)",
            params![id, "2026-02-20 10:00:00", format!("Conv {id}")],
        ).unwrap();

        for (_i, (role, content, timestamp)) in msgs.iter().enumerate() {
            let msg_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO messages (id, conversation_id, role, content, timestamp)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![msg_id, id, role, content, timestamp],
            ).unwrap();
        }
    }

    #[test]
    fn finds_eligible_conversation() {
        let conn = setup_db();
        let scheduler = SqliteBatchScheduler::new();

        // Insert a conversation with 2 messages, old enough (> 6 hours ago)
        insert_conversation(&conn, "conv-1", &[
            ("patient", "I have headaches", "2026-02-19 10:00:00"),
            ("coheara", "Tell me more", "2026-02-19 10:01:00"),
        ]);

        let config = ExtractionConfig::default();
        let eligible = scheduler.get_eligible_conversations(&conn, &config).unwrap();

        assert_eq!(eligible.len(), 1);
        assert_eq!(eligible[0].id, "conv-1");
        assert_eq!(eligible[0].messages.len(), 2);
        assert_eq!(eligible[0].message_count, 2);
    }

    #[test]
    fn skips_too_recent_conversation() {
        let conn = setup_db();
        let scheduler = SqliteBatchScheduler::new();

        // Insert a conversation with messages right now (too recent)
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        insert_conversation(&conn, "conv-recent", &[
            ("patient", "Hello", &now),
            ("coheara", "Hi", &now),
        ]);

        let config = ExtractionConfig::default();
        let eligible = scheduler.get_eligible_conversations(&conn, &config).unwrap();

        assert!(eligible.is_empty(), "Recent conversations should not be eligible");
    }

    #[test]
    fn skips_single_message_conversation() {
        let conn = setup_db();
        let scheduler = SqliteBatchScheduler::new();

        // Insert a conversation with only 1 message
        insert_conversation(&conn, "conv-single", &[
            ("patient", "Hello", "2026-02-19 10:00:00"),
        ]);

        let config = ExtractionConfig::default();
        let eligible = scheduler.get_eligible_conversations(&conn, &config).unwrap();

        assert!(eligible.is_empty(), "Single-message conversations should not be eligible");
    }

    #[test]
    fn skips_already_extracted_conversation() {
        let conn = setup_db();
        let scheduler = SqliteBatchScheduler::new();

        insert_conversation(&conn, "conv-extracted", &[
            ("patient", "I have headaches", "2026-02-19 10:00:00"),
            ("coheara", "Tell me more", "2026-02-19 10:01:00"),
        ]);

        // Mark as already extracted (after the last message)
        let batch_id = new_batch_id();
        scheduler.mark_extracted(
            &conn,
            "conv-extracted",
            &batch_id,
            &[ExtractionDomain::Symptom],
            1,
            "medgemma:4b",
            5000,
        ).unwrap();

        let config = ExtractionConfig::default();
        let eligible = scheduler.get_eligible_conversations(&conn, &config).unwrap();

        assert!(eligible.is_empty(), "Already-extracted conversations should not be eligible");
    }

    #[test]
    fn re_eligible_after_new_messages() {
        let conn = setup_db();
        let scheduler = SqliteBatchScheduler::new();

        insert_conversation(&conn, "conv-new", &[
            ("patient", "I have headaches", "2026-02-18 10:00:00"),
            ("coheara", "Tell me more", "2026-02-18 10:01:00"),
        ]);

        // Mark as extracted at an old timestamp
        conn.execute(
            "INSERT INTO extraction_batches (id, conversation_id, extracted_at, domains_found, items_extracted, model_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![new_batch_id(), "conv-new", "2026-02-18T10:05:00Z", "[\"symptom\"]", 1, "medgemma:4b"],
        ).unwrap();

        // Add a new message after extraction
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![Uuid::new_v4().to_string(), "conv-new", "patient", "The headache got worse", "2026-02-19 10:00:00"],
        ).unwrap();

        let config = ExtractionConfig::default();
        let eligible = scheduler.get_eligible_conversations(&conn, &config).unwrap();

        assert_eq!(eligible.len(), 1, "Should be re-eligible after new messages");
    }

    #[test]
    fn respects_max_conversations_limit() {
        let conn = setup_db();
        let scheduler = SqliteBatchScheduler::new();

        // Insert 5 eligible conversations
        for i in 0..5 {
            insert_conversation(&conn, &format!("conv-{i}"), &[
                ("patient", "Hello", "2026-02-19 10:00:00"),
                ("coheara", "Hi", "2026-02-19 10:01:00"),
            ]);
        }

        let mut config = ExtractionConfig::default();
        config.max_conversations_per_batch = 3;

        let eligible = scheduler.get_eligible_conversations(&conn, &config).unwrap();
        assert!(eligible.len() <= 3, "Should respect max_conversations_per_batch");
    }

    #[test]
    fn has_pending_work_returns_true() {
        let conn = setup_db();
        let scheduler = SqliteBatchScheduler::new();

        insert_conversation(&conn, "conv-1", &[
            ("patient", "Headaches", "2026-02-19 10:00:00"),
            ("coheara", "Tell me more", "2026-02-19 10:01:00"),
        ]);

        assert!(scheduler.has_pending_work(&conn).unwrap());
    }

    #[test]
    fn has_pending_work_returns_false_when_empty() {
        let conn = setup_db();
        let scheduler = SqliteBatchScheduler::new();
        assert!(!scheduler.has_pending_work(&conn).unwrap());
    }

    #[test]
    fn mark_extracted_creates_batch_record() {
        let conn = setup_db();
        let scheduler = SqliteBatchScheduler::new();

        insert_conversation(&conn, "conv-1", &[
            ("patient", "Test", "2026-02-19 10:00:00"),
            ("coheara", "Test", "2026-02-19 10:01:00"),
        ]);

        let batch_id = new_batch_id();
        scheduler.mark_extracted(
            &conn, "conv-1", &batch_id,
            &[ExtractionDomain::Symptom, ExtractionDomain::Medication],
            3, "medgemma:4b", 12000,
        ).unwrap();

        let count: u32 = conn.query_row(
            "SELECT COUNT(*) FROM extraction_batches WHERE conversation_id = 'conv-1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn messages_loaded_in_order() {
        let conn = setup_db();
        let scheduler = SqliteBatchScheduler::new();

        insert_conversation(&conn, "conv-ordered", &[
            ("patient", "First message", "2026-02-19 10:00:00"),
            ("coheara", "Second message", "2026-02-19 10:01:00"),
            ("patient", "Third message", "2026-02-19 10:02:00"),
        ]);

        let config = ExtractionConfig::default();
        let eligible = scheduler.get_eligible_conversations(&conn, &config).unwrap();

        assert_eq!(eligible.len(), 1);
        let messages = &eligible[0].messages;
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].content, "First message");
        assert_eq!(messages[1].content, "Second message");
        assert_eq!(messages[2].content, "Third message");
        assert_eq!(messages[0].index, 0);
        assert_eq!(messages[2].index, 2);
    }
}
