//! M4: Pending Review Store — CRUD for extraction_pending table.
//!
//! Holds extracted items between batch processing and user review.
//! Items persist until explicitly confirmed or dismissed.

use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

use super::error::ExtractionError;
use super::traits::PendingReviewStore;
use super::types::*;
use crate::db::DatabaseError;

/// SQLite-backed pending review store.
pub struct SqlitePendingStore;

impl SqlitePendingStore {
    pub fn new() -> Self {
        Self
    }
}

impl PendingReviewStore for SqlitePendingStore {
    fn store_pending(
        &self,
        conn: &Connection,
        items: &[PendingReviewItem],
    ) -> Result<(), ExtractionError> {
        let tx = conn.unchecked_transaction()
            .map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;

        for item in items {
            let source_ids_json = serde_json::to_string(&item.source_message_ids)
                .map_err(|e| ExtractionError::JsonParsing(e.to_string()))?;
            let extracted_json = serde_json::to_string(&item.extracted_data)
                .map_err(|e| ExtractionError::JsonParsing(e.to_string()))?;

            tx.execute(
                "INSERT INTO extraction_pending
                 (id, conversation_id, batch_id, domain, extracted_data, confidence,
                  grounding, duplicate_of, source_message_ids, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    item.id,
                    item.conversation_id,
                    item.batch_id,
                    item.domain.as_str(),
                    extracted_json,
                    item.confidence,
                    item.grounding.as_str(),
                    item.duplicate_of,
                    source_ids_json,
                    item.status.as_str(),
                    item.created_at,
                ],
            ).map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;
        }

        tx.commit()
            .map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;

        Ok(())
    }

    fn get_pending(
        &self,
        conn: &Connection,
    ) -> Result<Vec<PendingReviewItem>, ExtractionError> {
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, batch_id, domain, extracted_data, confidence,
                    grounding, duplicate_of, source_message_ids, status, created_at, reviewed_at
             FROM extraction_pending
             WHERE status = 'pending'
             ORDER BY created_at ASC"
        ).map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;

        let rows = stmt.query_map([], |row| {
            Ok(PendingRow {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                batch_id: row.get(2)?,
                domain: row.get(3)?,
                extracted_data: row.get(4)?,
                confidence: row.get(5)?,
                grounding: row.get(6)?,
                duplicate_of: row.get(7)?,
                source_message_ids: row.get(8)?,
                status: row.get(9)?,
                created_at: row.get(10)?,
                reviewed_at: row.get(11)?,
            })
        }).map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;

        let mut items = Vec::new();
        for row in rows {
            let row = row.map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;
            items.push(pending_from_row(row)?);
        }
        Ok(items)
    }

    fn get_pending_count(
        &self,
        conn: &Connection,
    ) -> Result<u32, ExtractionError> {
        let count: u32 = conn.query_row(
            "SELECT COUNT(*) FROM extraction_pending WHERE status = 'pending'",
            [],
            |row| row.get(0),
        ).map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;
        Ok(count)
    }

    fn confirm_item(
        &self,
        conn: &Connection,
        item_id: &str,
    ) -> Result<PendingReviewItem, ExtractionError> {
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        conn.execute(
            "UPDATE extraction_pending SET status = 'confirmed', reviewed_at = ?1 WHERE id = ?2",
            params![now, item_id],
        ).map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;

        self.get_item_by_id(conn, item_id)
    }

    fn confirm_item_with_edits(
        &self,
        conn: &Connection,
        item_id: &str,
        edits: serde_json::Value,
    ) -> Result<PendingReviewItem, ExtractionError> {
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let edits_json = serde_json::to_string(&edits)
            .map_err(|e| ExtractionError::JsonParsing(e.to_string()))?;

        conn.execute(
            "UPDATE extraction_pending
             SET status = 'edited_confirmed', reviewed_at = ?1, extracted_data = ?2
             WHERE id = ?3",
            params![now, edits_json, item_id],
        ).map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;

        self.get_item_by_id(conn, item_id)
    }

    fn dismiss_item(
        &self,
        conn: &Connection,
        item_id: &str,
    ) -> Result<(), ExtractionError> {
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        conn.execute(
            "UPDATE extraction_pending SET status = 'dismissed', reviewed_at = ?1 WHERE id = ?2",
            params![now, item_id],
        ).map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;

        Ok(())
    }

    fn dismiss_items(
        &self,
        conn: &Connection,
        item_ids: &[String],
    ) -> Result<(), ExtractionError> {
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        for id in item_ids {
            conn.execute(
                "UPDATE extraction_pending SET status = 'dismissed', reviewed_at = ?1 WHERE id = ?2",
                params![now, id],
            ).map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;
        }

        Ok(())
    }
}

impl SqlitePendingStore {
    /// Fetch a single pending item by ID. Used by IPC commands for dispatch.
    pub fn get_item_by_id(
        &self,
        conn: &Connection,
        item_id: &str,
    ) -> Result<PendingReviewItem, ExtractionError> {
        let row = conn.query_row(
            "SELECT id, conversation_id, batch_id, domain, extracted_data, confidence,
                    grounding, duplicate_of, source_message_ids, status, created_at, reviewed_at
             FROM extraction_pending WHERE id = ?1",
            params![item_id],
            |row| {
                Ok(PendingRow {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    batch_id: row.get(2)?,
                    domain: row.get(3)?,
                    extracted_data: row.get(4)?,
                    confidence: row.get(5)?,
                    grounding: row.get(6)?,
                    duplicate_of: row.get(7)?,
                    source_message_ids: row.get(8)?,
                    status: row.get(9)?,
                    created_at: row.get(10)?,
                    reviewed_at: row.get(11)?,
                })
            },
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                ExtractionError::Database(DatabaseError::NotFound {
                    entity_type: "extraction_pending".to_string(),
                    id: item_id.to_string(),
                })
            }
            _ => ExtractionError::Database(DatabaseError::Sqlite(e)),
        })?;

        pending_from_row(row)
    }
}

/// Create a PendingReviewItem with generated ID and current timestamp.
pub fn create_pending_item(
    conversation_id: &str,
    batch_id: &str,
    domain: ExtractionDomain,
    extracted_data: serde_json::Value,
    confidence: f32,
    grounding: Grounding,
    duplicate_of: Option<String>,
    source_message_ids: Vec<String>,
) -> PendingReviewItem {
    PendingReviewItem {
        id: Uuid::new_v4().to_string(),
        conversation_id: conversation_id.to_string(),
        batch_id: batch_id.to_string(),
        domain,
        extracted_data,
        confidence,
        grounding,
        duplicate_of,
        source_message_ids,
        status: PendingStatus::Pending,
        created_at: Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        reviewed_at: None,
    }
}

// ═══════════════════════════════════════════
// Internal row mapping
// ═══════════════════════════════════════════

struct PendingRow {
    id: String,
    conversation_id: String,
    batch_id: String,
    domain: String,
    extracted_data: String,
    confidence: f64,
    grounding: String,
    duplicate_of: Option<String>,
    source_message_ids: String,
    status: String,
    created_at: String,
    reviewed_at: Option<String>,
}

fn pending_from_row(row: PendingRow) -> Result<PendingReviewItem, ExtractionError> {
    let domain = ExtractionDomain::from_str(&row.domain)
        .ok_or_else(|| ExtractionError::JsonParsing(format!("Unknown domain: {}", row.domain)))?;
    let grounding = Grounding::from_str(&row.grounding)
        .ok_or_else(|| ExtractionError::JsonParsing(format!("Unknown grounding: {}", row.grounding)))?;
    let status = PendingStatus::from_str(&row.status)
        .ok_or_else(|| ExtractionError::JsonParsing(format!("Unknown status: {}", row.status)))?;

    let extracted_data: serde_json::Value = serde_json::from_str(&row.extracted_data)
        .map_err(|e| ExtractionError::JsonParsing(format!("Bad extracted_data JSON: {e}")))?;
    let source_message_ids: Vec<String> = serde_json::from_str(&row.source_message_ids)
        .map_err(|e| ExtractionError::JsonParsing(format!("Bad source_message_ids JSON: {e}")))?;

    Ok(PendingReviewItem {
        id: row.id,
        conversation_id: row.conversation_id,
        batch_id: row.batch_id,
        domain,
        extracted_data,
        confidence: row.confidence as f32,
        grounding,
        duplicate_of: row.duplicate_of,
        source_message_ids,
        status,
        created_at: row.created_at,
        reviewed_at: row.reviewed_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;

    fn setup_db() -> Connection {
        let conn = open_memory_database().expect("Failed to open in-memory DB");
        conn
    }

    fn make_pending_item(domain: ExtractionDomain) -> PendingReviewItem {
        create_pending_item(
            "conv-1",
            "batch-1",
            domain,
            serde_json::json!({"category": "Pain", "specific": "Headache"}),
            0.85,
            Grounding::Grounded,
            None,
            vec!["msg-0".to_string(), "msg-2".to_string()],
        )
    }

    #[test]
    fn store_and_retrieve_pending() {
        let conn = setup_db();
        let store = SqlitePendingStore::new();

        // Need a conversation first (FK constraint)
        conn.execute(
            "INSERT INTO conversations (id, started_at) VALUES ('conv-1', '2026-02-20 10:00:00')",
            [],
        ).unwrap();

        // Need a batch record first (FK constraint)
        conn.execute(
            "INSERT INTO extraction_batches (id, conversation_id, extracted_at, domains_found, items_extracted, model_name)
             VALUES ('batch-1', 'conv-1', '2026-02-20T22:00:00Z', '[\"symptom\"]', 1, 'medgemma:4b')",
            [],
        ).unwrap();

        let item = make_pending_item(ExtractionDomain::Symptom);
        store.store_pending(&conn, &[item.clone()]).unwrap();

        let pending = store.get_pending(&conn).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].domain, ExtractionDomain::Symptom);
        assert_eq!(pending[0].confidence, 0.85);
        assert_eq!(pending[0].grounding, Grounding::Grounded);
        assert_eq!(pending[0].status, PendingStatus::Pending);
    }

    #[test]
    fn get_pending_count() {
        let conn = setup_db();
        let store = SqlitePendingStore::new();

        conn.execute(
            "INSERT INTO conversations (id, started_at) VALUES ('conv-1', '2026-02-20 10:00:00')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO extraction_batches (id, conversation_id, extracted_at, domains_found, items_extracted, model_name)
             VALUES ('batch-1', 'conv-1', '2026-02-20T22:00:00Z', '[\"symptom\"]', 2, 'medgemma:4b')",
            [],
        ).unwrap();

        let item1 = make_pending_item(ExtractionDomain::Symptom);
        let item2 = make_pending_item(ExtractionDomain::Medication);
        store.store_pending(&conn, &[item1, item2]).unwrap();

        assert_eq!(store.get_pending_count(&conn).unwrap(), 2);
    }

    #[test]
    fn confirm_item_updates_status() {
        let conn = setup_db();
        let store = SqlitePendingStore::new();

        conn.execute(
            "INSERT INTO conversations (id, started_at) VALUES ('conv-1', '2026-02-20 10:00:00')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO extraction_batches (id, conversation_id, extracted_at, domains_found, items_extracted, model_name)
             VALUES ('batch-1', 'conv-1', '2026-02-20T22:00:00Z', '[\"symptom\"]', 1, 'medgemma:4b')",
            [],
        ).unwrap();

        let item = make_pending_item(ExtractionDomain::Symptom);
        let item_id = item.id.clone();
        store.store_pending(&conn, &[item]).unwrap();

        let confirmed = store.confirm_item(&conn, &item_id).unwrap();
        assert_eq!(confirmed.status, PendingStatus::Confirmed);
        assert!(confirmed.reviewed_at.is_some());

        // Should no longer appear in pending list
        assert_eq!(store.get_pending_count(&conn).unwrap(), 0);
    }

    #[test]
    fn confirm_with_edits() {
        let conn = setup_db();
        let store = SqlitePendingStore::new();

        conn.execute(
            "INSERT INTO conversations (id, started_at) VALUES ('conv-1', '2026-02-20 10:00:00')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO extraction_batches (id, conversation_id, extracted_at, domains_found, items_extracted, model_name)
             VALUES ('batch-1', 'conv-1', '2026-02-20T22:00:00Z', '[\"symptom\"]', 1, 'medgemma:4b')",
            [],
        ).unwrap();

        let item = make_pending_item(ExtractionDomain::Symptom);
        let item_id = item.id.clone();
        store.store_pending(&conn, &[item]).unwrap();

        let edits = serde_json::json!({"category": "Pain", "specific": "Migraine", "severity_hint": 5});
        let edited = store.confirm_item_with_edits(&conn, &item_id, edits).unwrap();
        assert_eq!(edited.status, PendingStatus::EditedConfirmed);
        assert_eq!(edited.extracted_data["specific"], "Migraine");
    }

    #[test]
    fn dismiss_item_updates_status() {
        let conn = setup_db();
        let store = SqlitePendingStore::new();

        conn.execute(
            "INSERT INTO conversations (id, started_at) VALUES ('conv-1', '2026-02-20 10:00:00')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO extraction_batches (id, conversation_id, extracted_at, domains_found, items_extracted, model_name)
             VALUES ('batch-1', 'conv-1', '2026-02-20T22:00:00Z', '[\"symptom\"]', 1, 'medgemma:4b')",
            [],
        ).unwrap();

        let item = make_pending_item(ExtractionDomain::Symptom);
        let item_id = item.id.clone();
        store.store_pending(&conn, &[item]).unwrap();

        store.dismiss_item(&conn, &item_id).unwrap();

        // Should no longer appear in pending list
        assert_eq!(store.get_pending_count(&conn).unwrap(), 0);
    }

    #[test]
    fn dismiss_multiple_items() {
        let conn = setup_db();
        let store = SqlitePendingStore::new();

        conn.execute(
            "INSERT INTO conversations (id, started_at) VALUES ('conv-1', '2026-02-20 10:00:00')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO extraction_batches (id, conversation_id, extracted_at, domains_found, items_extracted, model_name)
             VALUES ('batch-1', 'conv-1', '2026-02-20T22:00:00Z', '[\"symptom\"]', 2, 'medgemma:4b')",
            [],
        ).unwrap();

        let item1 = make_pending_item(ExtractionDomain::Symptom);
        let item2 = make_pending_item(ExtractionDomain::Medication);
        let ids = vec![item1.id.clone(), item2.id.clone()];
        store.store_pending(&conn, &[item1, item2]).unwrap();

        store.dismiss_items(&conn, &ids).unwrap();
        assert_eq!(store.get_pending_count(&conn).unwrap(), 0);
    }

    #[test]
    fn create_pending_item_generates_id() {
        let item1 = create_pending_item("c1", "b1", ExtractionDomain::Symptom, serde_json::json!({}), 0.5, Grounding::Partial, None, vec![]);
        let item2 = create_pending_item("c1", "b1", ExtractionDomain::Symptom, serde_json::json!({}), 0.5, Grounding::Partial, None, vec![]);
        assert_ne!(item1.id, item2.id, "Each item should get a unique ID");
    }

    #[test]
    fn store_multiple_items_atomically() {
        let conn = setup_db();
        let store = SqlitePendingStore::new();

        conn.execute(
            "INSERT INTO conversations (id, started_at) VALUES ('conv-1', '2026-02-20 10:00:00')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO extraction_batches (id, conversation_id, extracted_at, domains_found, items_extracted, model_name)
             VALUES ('batch-1', 'conv-1', '2026-02-20T22:00:00Z', '[\"symptom\",\"medication\",\"appointment\"]', 3, 'medgemma:4b')",
            [],
        ).unwrap();

        let items: Vec<PendingReviewItem> = ExtractionDomain::all()
            .iter()
            .map(|&d| make_pending_item(d))
            .collect();

        store.store_pending(&conn, &items).unwrap();
        assert_eq!(store.get_pending_count(&conn).unwrap(), 3);
    }
}
