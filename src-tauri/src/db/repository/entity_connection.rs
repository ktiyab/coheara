//! BTL-10 C2: Repository functions for processing_log and entity_connections.

use std::str::FromStr;

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::DatabaseError;
use crate::models::entity_connection::*;

// ---------------------------------------------------------------------------
// Processing log
// ---------------------------------------------------------------------------

/// Insert a processing log entry.
pub fn insert_processing_log(
    conn: &Connection,
    entry: &ProcessingLogEntry,
) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO processing_log (id, document_id, model_name, model_variant, processing_stage, started_at, completed_at, success, error_message)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            entry.id.to_string(),
            entry.document_id.to_string(),
            entry.model_name,
            entry.model_variant,
            entry.processing_stage.as_str(),
            entry.started_at,
            entry.completed_at,
            entry.success as i32,
            entry.error_message,
        ],
    )?;
    Ok(())
}

/// Get all processing log entries for a document, ordered by started_at.
pub fn get_processing_log_for_document(
    conn: &Connection,
    document_id: &Uuid,
) -> Result<Vec<ProcessingLogEntry>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, document_id, model_name, model_variant, processing_stage, started_at, completed_at, success, error_message
         FROM processing_log
         WHERE document_id = ?1
         ORDER BY started_at ASC",
    )?;

    let rows = stmt.query_map(params![document_id.to_string()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, bool>(7)?,
            row.get::<_, Option<String>>(8)?,
        ))
    })?;

    let mut entries = Vec::new();
    for row in rows {
        let (id, doc_id, model_name, model_variant, stage, started_at, completed_at, success, error_message) = row?;
        entries.push(ProcessingLogEntry {
            id: Uuid::parse_str(&id).map_err(|e| DatabaseError::InvalidData(e.to_string()))?,
            document_id: Uuid::parse_str(&doc_id).map_err(|e| DatabaseError::InvalidData(e.to_string()))?,
            model_name,
            model_variant,
            processing_stage: ProcessingStage::from_str(&stage)?,
            started_at,
            completed_at,
            success,
            error_message,
        });
    }
    Ok(entries)
}

/// Get the last error message from processing_log for a document.
pub fn get_last_processing_error(
    conn: &Connection,
    document_id: &Uuid,
) -> Result<Option<String>, DatabaseError> {
    let result = conn.query_row(
        "SELECT error_message FROM processing_log
         WHERE document_id = ?1 AND success = 0 AND error_message IS NOT NULL
         ORDER BY started_at DESC LIMIT 1",
        params![document_id.to_string()],
        |row| row.get::<_, String>(0),
    );

    match result {
        Ok(msg) => Ok(Some(msg)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(DatabaseError::Sqlite(e)),
    }
}

// ---------------------------------------------------------------------------
// Entity connections
// ---------------------------------------------------------------------------

/// Insert an entity connection.
pub fn insert_entity_connection(
    conn: &Connection,
    connection: &EntityConnection,
) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO entity_connections (id, source_type, source_id, target_type, target_id, relationship_type, confidence, document_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            connection.id.to_string(),
            connection.source_type.as_str(),
            connection.source_id.to_string(),
            connection.target_type.as_str(),
            connection.target_id.to_string(),
            connection.relationship_type.as_str(),
            connection.confidence,
            connection.document_id.to_string(),
        ],
    )?;
    Ok(())
}

/// Get all entity connections for a document.
pub fn get_connections_for_document(
    conn: &Connection,
    document_id: &Uuid,
) -> Result<Vec<EntityConnection>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, source_type, source_id, target_type, target_id, relationship_type, confidence, document_id, created_at
         FROM entity_connections
         WHERE document_id = ?1
         ORDER BY created_at ASC",
    )?;

    parse_connection_rows(&mut stmt, params![document_id.to_string()])
}

/// Get all entity connections where a specific entity is source or target.
pub fn get_connections_for_entity(
    conn: &Connection,
    entity_type: &EntityType,
    entity_id: &Uuid,
) -> Result<Vec<EntityConnection>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, source_type, source_id, target_type, target_id, relationship_type, confidence, document_id, created_at
         FROM entity_connections
         WHERE (source_type = ?1 AND source_id = ?2) OR (target_type = ?1 AND target_id = ?2)
         ORDER BY created_at ASC",
    )?;

    parse_connection_rows(&mut stmt, params![entity_type.as_str(), entity_id.to_string()])
}

/// Delete all entity connections for a document.
pub fn delete_connections_for_document(
    conn: &Connection,
    document_id: &Uuid,
) -> Result<u64, DatabaseError> {
    let rows = conn.execute(
        "DELETE FROM entity_connections WHERE document_id = ?1",
        params![document_id.to_string()],
    )?;
    Ok(rows as u64)
}

// ---------------------------------------------------------------------------
// Internal helper
// ---------------------------------------------------------------------------

fn parse_connection_rows(
    stmt: &mut rusqlite::Statement,
    params: impl rusqlite::Params,
) -> Result<Vec<EntityConnection>, DatabaseError> {
    let rows = stmt.query_map(params, |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, f64>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, String>(8)?,
        ))
    })?;

    let mut connections = Vec::new();
    for row in rows {
        let (id, src_type, src_id, tgt_type, tgt_id, rel_type, confidence, doc_id, created_at) = row?;
        connections.push(EntityConnection {
            id: Uuid::parse_str(&id).map_err(|e| DatabaseError::InvalidData(e.to_string()))?,
            source_type: EntityType::from_str(&src_type)?,
            source_id: Uuid::parse_str(&src_id).map_err(|e| DatabaseError::InvalidData(e.to_string()))?,
            target_type: EntityType::from_str(&tgt_type)?,
            target_id: Uuid::parse_str(&tgt_id).map_err(|e| DatabaseError::InvalidData(e.to_string()))?,
            relationship_type: RelationshipType::from_str(&rel_type)?,
            confidence,
            document_id: Uuid::parse_str(&doc_id).map_err(|e| DatabaseError::InvalidData(e.to_string()))?,
            created_at,
        });
    }
    Ok(connections)
}
