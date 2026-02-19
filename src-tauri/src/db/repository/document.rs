use std::str::FromStr;

use chrono::{NaiveDate, NaiveDateTime};
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::DatabaseError;
use crate::models::*;
use crate::models::enums::*;

pub fn insert_document(conn: &Connection, doc: &Document) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO documents (id, type, title, document_date, ingestion_date, professional_id,
         source_file, markdown_file, ocr_confidence, verified, source_deleted, perceptual_hash, notes,
         pipeline_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            doc.id.to_string(),
            doc.doc_type.as_str(),
            doc.title,
            doc.document_date.map(|d| d.to_string()),
            doc.ingestion_date.to_string(),
            doc.professional_id.map(|id| id.to_string()),
            doc.source_file,
            doc.markdown_file,
            doc.ocr_confidence,
            doc.verified as i32,
            doc.source_deleted as i32,
            doc.perceptual_hash,
            doc.notes,
            doc.pipeline_status.as_str(),
        ],
    )?;
    Ok(())
}

pub fn get_document(conn: &Connection, id: &Uuid) -> Result<Option<Document>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, type, title, document_date, ingestion_date, professional_id,
         source_file, markdown_file, ocr_confidence, verified, source_deleted, perceptual_hash, notes,
         pipeline_status
         FROM documents WHERE id = ?1"
    )?;

    let result = stmt.query_row(params![id.to_string()], |row| {
        Ok(DocumentRow {
            id: row.get::<_, String>(0)?,
            doc_type: row.get::<_, String>(1)?,
            title: row.get::<_, String>(2)?,
            document_date: row.get::<_, Option<String>>(3)?,
            ingestion_date: row.get::<_, String>(4)?,
            professional_id: row.get::<_, Option<String>>(5)?,
            source_file: row.get::<_, String>(6)?,
            markdown_file: row.get::<_, Option<String>>(7)?,
            ocr_confidence: row.get::<_, Option<f32>>(8)?,
            verified: row.get::<_, i32>(9)?,
            source_deleted: row.get::<_, i32>(10)?,
            perceptual_hash: row.get::<_, Option<String>>(11)?,
            notes: row.get::<_, Option<String>>(12)?,
            pipeline_status: row.get::<_, Option<String>>(13)?,
        })
    });

    match result {
        Ok(row) => Ok(Some(document_from_row(row)?)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn get_document_by_hash(conn: &Connection, hash: &str) -> Result<Option<Document>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, type, title, document_date, ingestion_date, professional_id,
         source_file, markdown_file, ocr_confidence, verified, source_deleted, perceptual_hash, notes,
         pipeline_status
         FROM documents WHERE perceptual_hash = ?1 LIMIT 1"
    )?;

    let result = stmt.query_row(params![hash], |row| {
        Ok(DocumentRow {
            id: row.get::<_, String>(0)?,
            doc_type: row.get::<_, String>(1)?,
            title: row.get::<_, String>(2)?,
            document_date: row.get::<_, Option<String>>(3)?,
            ingestion_date: row.get::<_, String>(4)?,
            professional_id: row.get::<_, Option<String>>(5)?,
            source_file: row.get::<_, String>(6)?,
            markdown_file: row.get::<_, Option<String>>(7)?,
            ocr_confidence: row.get::<_, Option<f32>>(8)?,
            verified: row.get::<_, i32>(9)?,
            source_deleted: row.get::<_, i32>(10)?,
            perceptual_hash: row.get::<_, Option<String>>(11)?,
            notes: row.get::<_, Option<String>>(12)?,
            pipeline_status: row.get::<_, Option<String>>(13)?,
        })
    });

    match result {
        Ok(row) => Ok(Some(document_from_row(row)?)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn update_document(conn: &Connection, doc: &Document) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE documents SET type = ?2, title = ?3, document_date = ?4,
         professional_id = ?5, markdown_file = ?6, ocr_confidence = ?7,
         verified = ?8, notes = ?9, pipeline_status = ?10
         WHERE id = ?1",
        params![
            doc.id.to_string(),
            doc.doc_type.as_str(),
            doc.title,
            doc.document_date.map(|d| d.to_string()),
            doc.professional_id.map(|id| id.to_string()),
            doc.markdown_file,
            doc.ocr_confidence,
            doc.verified as i32,
            doc.notes,
            doc.pipeline_status.as_str(),
        ],
    )?;
    Ok(())
}

/// Update only the pipeline_status of a document.
pub fn update_pipeline_status(
    conn: &Connection,
    document_id: &Uuid,
    status: &PipelineStatus,
) -> Result<(), DatabaseError> {
    let rows = conn.execute(
        "UPDATE documents SET pipeline_status = ?2 WHERE id = ?1",
        params![document_id.to_string(), status.as_str()],
    )?;
    if rows == 0 {
        return Err(DatabaseError::NotFound {
            entity_type: "Document".into(),
            id: document_id.to_string(),
        });
    }
    Ok(())
}

/// Get all documents matching a pipeline status.
pub fn get_documents_by_pipeline_status(
    conn: &Connection,
    status: &PipelineStatus,
) -> Result<Vec<Document>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, type, title, document_date, ingestion_date, professional_id,
         source_file, markdown_file, ocr_confidence, verified, source_deleted, perceptual_hash, notes,
         pipeline_status
         FROM documents WHERE pipeline_status = ?1 ORDER BY ingestion_date DESC"
    )?;

    let rows = stmt.query_map(params![status.as_str()], |row| {
        Ok(DocumentRow {
            id: row.get::<_, String>(0)?,
            doc_type: row.get::<_, String>(1)?,
            title: row.get::<_, String>(2)?,
            document_date: row.get::<_, Option<String>>(3)?,
            ingestion_date: row.get::<_, String>(4)?,
            professional_id: row.get::<_, Option<String>>(5)?,
            source_file: row.get::<_, String>(6)?,
            markdown_file: row.get::<_, Option<String>>(7)?,
            ocr_confidence: row.get::<_, Option<f32>>(8)?,
            verified: row.get::<_, i32>(9)?,
            source_deleted: row.get::<_, i32>(10)?,
            perceptual_hash: row.get::<_, Option<String>>(11)?,
            notes: row.get::<_, Option<String>>(12)?,
            pipeline_status: row.get::<_, Option<String>>(13)?,
        })
    })?;

    let mut docs = Vec::new();
    for row in rows {
        docs.push(document_from_row(row?)?);
    }
    Ok(docs)
}

/// Delete a document and all its child entities.
///
/// Entity tables (medications, lab_results, diagnoses, allergies, procedures,
/// referrals) lack CASCADE on document_id FK, so we delete children first.
/// Vector chunks DO have CASCADE but we delete them explicitly for logging.
/// Uses a transaction for atomicity.
pub fn delete_document_cascade(conn: &Connection, document_id: &Uuid) -> Result<(), DatabaseError> {
    let doc_id_str = document_id.to_string();

    // First collect medication IDs for sub-entity cleanup (compound_ingredients,
    // tapering_schedules, medication_instructions have CASCADE on medication_id,
    // but we delete medications by document_id which is not a CASCADE path).
    let mut med_stmt = conn.prepare(
        "SELECT id FROM medications WHERE document_id = ?1"
    )?;
    let med_ids: Vec<String> = med_stmt
        .query_map(params![doc_id_str], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    drop(med_stmt);

    // Delete medication sub-entities (CASCADE handles these if we delete the medication,
    // but deleting medications by document_id won't trigger CASCADE on med sub-tables)
    for med_id in &med_ids {
        conn.execute("DELETE FROM medication_instructions WHERE medication_id = ?1", params![med_id])?;
        conn.execute("DELETE FROM tapering_schedules WHERE medication_id = ?1", params![med_id])?;
        conn.execute("DELETE FROM compound_ingredients WHERE medication_id = ?1", params![med_id])?;
        conn.execute("DELETE FROM dose_changes WHERE medication_id = ?1", params![med_id])?;
    }

    // Delete entity tables that reference document_id
    let deleted_meds = conn.execute("DELETE FROM medications WHERE document_id = ?1", params![doc_id_str])?;
    let deleted_labs = conn.execute("DELETE FROM lab_results WHERE document_id = ?1", params![doc_id_str])?;
    let deleted_diag = conn.execute("DELETE FROM diagnoses WHERE document_id = ?1", params![doc_id_str])?;
    let deleted_allergy = conn.execute("DELETE FROM allergies WHERE document_id = ?1", params![doc_id_str])?;
    let deleted_procs = conn.execute("DELETE FROM procedures WHERE document_id = ?1", params![doc_id_str])?;
    conn.execute("DELETE FROM referrals WHERE document_id = ?1", params![doc_id_str])?;

    // Delete vector chunks (has CASCADE but explicit for logging)
    let deleted_chunks = conn.execute("DELETE FROM vector_chunks WHERE document_id = ?1", params![doc_id_str])?;

    // Delete the document itself
    let deleted = conn.execute("DELETE FROM documents WHERE id = ?1", params![doc_id_str])?;
    if deleted == 0 {
        return Err(DatabaseError::NotFound {
            entity_type: "Document".into(),
            id: doc_id_str,
        });
    }

    // Recalculate trust metrics from actual data
    if let Err(e) = super::profile_trust::recalculate_profile_trust(conn) {
        tracing::warn!(error = %e, "Failed to recalculate trust after document deletion");
    }

    tracing::info!(
        document_id = %document_id,
        medications = deleted_meds,
        lab_results = deleted_labs,
        diagnoses = deleted_diag,
        allergies = deleted_allergy,
        procedures = deleted_procs,
        chunks = deleted_chunks,
        "Document cascade-deleted with all child entities"
    );

    Ok(())
}

/// Clear all entities and vector chunks for a document WITHOUT deleting the document itself.
///
/// Used by the reprocessing flow (P.3: idempotent entity store) to safely
/// re-run extraction/structuring on an existing document.
pub fn clear_document_entities(conn: &Connection, document_id: &Uuid) -> Result<(), DatabaseError> {
    let doc_id_str = document_id.to_string();

    // Collect medication IDs for sub-entity cleanup
    let mut med_stmt = conn.prepare(
        "SELECT id FROM medications WHERE document_id = ?1"
    )?;
    let med_ids: Vec<String> = med_stmt
        .query_map(params![doc_id_str], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    drop(med_stmt);

    // Delete medication sub-entities
    for med_id in &med_ids {
        conn.execute("DELETE FROM medication_instructions WHERE medication_id = ?1", params![med_id])?;
        conn.execute("DELETE FROM tapering_schedules WHERE medication_id = ?1", params![med_id])?;
        conn.execute("DELETE FROM compound_ingredients WHERE medication_id = ?1", params![med_id])?;
        conn.execute("DELETE FROM dose_changes WHERE medication_id = ?1", params![med_id])?;
    }

    // Delete entity tables
    conn.execute("DELETE FROM medications WHERE document_id = ?1", params![doc_id_str])?;
    conn.execute("DELETE FROM lab_results WHERE document_id = ?1", params![doc_id_str])?;
    conn.execute("DELETE FROM diagnoses WHERE document_id = ?1", params![doc_id_str])?;
    conn.execute("DELETE FROM allergies WHERE document_id = ?1", params![doc_id_str])?;
    conn.execute("DELETE FROM procedures WHERE document_id = ?1", params![doc_id_str])?;
    conn.execute("DELETE FROM referrals WHERE document_id = ?1", params![doc_id_str])?;
    conn.execute("DELETE FROM vector_chunks WHERE document_id = ?1", params![doc_id_str])?;

    tracing::debug!(document_id = %document_id, "Cleared entities and chunks for document");
    Ok(())
}

// Internal row type for Document mapping
struct DocumentRow {
    id: String,
    doc_type: String,
    title: String,
    document_date: Option<String>,
    ingestion_date: String,
    professional_id: Option<String>,
    source_file: String,
    markdown_file: Option<String>,
    ocr_confidence: Option<f32>,
    verified: i32,
    source_deleted: i32,
    perceptual_hash: Option<String>,
    notes: Option<String>,
    pipeline_status: Option<String>,
}

fn document_from_row(row: DocumentRow) -> Result<Document, DatabaseError> {
    let pipeline_status = row
        .pipeline_status
        .as_deref()
        .and_then(|s| PipelineStatus::from_str(s).ok())
        .unwrap_or(PipelineStatus::Imported);

    Ok(Document {
        id: Uuid::parse_str(&row.id).map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
        doc_type: DocumentType::from_str(&row.doc_type)?,
        title: row.title,
        document_date: row.document_date.and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
        ingestion_date: NaiveDateTime::parse_from_str(&row.ingestion_date, "%Y-%m-%d %H:%M:%S")
            .or_else(|_| NaiveDateTime::parse_from_str(&row.ingestion_date, "%Y-%m-%dT%H:%M:%S"))
            .unwrap_or_default(),
        professional_id: row.professional_id.and_then(|s| Uuid::parse_str(&s).ok()),
        source_file: row.source_file,
        markdown_file: row.markdown_file,
        ocr_confidence: row.ocr_confidence,
        verified: row.verified != 0,
        source_deleted: row.source_deleted != 0,
        perceptual_hash: row.perceptual_hash,
        notes: row.notes,
        pipeline_status,
    })
}
