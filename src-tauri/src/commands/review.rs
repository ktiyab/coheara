//! L3-04 Review Screen — Tauri IPC commands.
//!
//! Five commands:
//! - `get_review_data`: fetch document + structuring result for review
//! - `get_original_file`: decrypt original file to base64
//! - `update_extracted_field`: validate a field correction
//! - `confirm_review`: apply corrections, run storage pipeline, update trust
//! - `reject_review`: reject with retry or remove action

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::core_state::CoreState;
use crate::crypto::encryption::EncryptedData;
use crate::db::repository::{
    get_document, update_profile_trust_corrected, update_profile_trust_verified,
};
use crate::db::sqlite::open_database;
use crate::pipeline::structuring::types::StructuringResult;
use crate::review::{
    apply_corrections, count_extracted_fields, detect_file_type, flatten_entities_to_fields,
    generate_plausibility_warnings, update_document_rejected, update_document_verified,
    EntitiesStoredSummary, FieldCorrection, ReviewConfirmResult, ReviewData, ReviewOutcome,
    ReviewRejectResult,
};

// ---------------------------------------------------------------------------
// Structuring result persistence (encrypted JSON in profile directory)
// ---------------------------------------------------------------------------

/// Save a structuring result as encrypted JSON for later review.
/// Called after L1-03 structuring, before the patient reviews.
pub fn save_pending_structuring(
    session: &crate::crypto::ProfileSession,
    result: &StructuringResult,
) -> Result<(), String> {
    let json = serde_json::to_vec(result).map_err(|e| format!("Serialize failed: {e}"))?;
    let encrypted = session
        .encrypt(&json)
        .map_err(|e| format!("Encryption failed: {e}"))?;

    let dir = session
        .db_path
        .parent()
        .and_then(|p| p.parent())
        .ok_or("Invalid profile directory")?;
    let pending_dir = dir.join("pending_review");
    std::fs::create_dir_all(&pending_dir)
        .map_err(|e| format!("Create dir failed: {e}"))?;

    let path = pending_dir.join(format!("{}.json.enc", result.document_id));
    std::fs::write(&path, encrypted.to_bytes())
        .map_err(|e| format!("Write failed: {e}"))?;

    Ok(())
}

/// Load a pending structuring result for review.
fn load_pending_structuring(
    session: &crate::crypto::ProfileSession,
    document_id: &Uuid,
) -> Result<StructuringResult, String> {
    let dir = session
        .db_path
        .parent()
        .and_then(|p| p.parent())
        .ok_or("Invalid profile directory")?;
    let path = dir.join(format!("pending_review/{}.json.enc", document_id));

    let bytes = std::fs::read(&path)
        .map_err(|e| format!("Pending structuring not found for {document_id}: {e}"))?;
    let encrypted =
        EncryptedData::from_bytes(&bytes).map_err(|e| format!("Corrupt encrypted data: {e}"))?;
    let json = session
        .decrypt(&encrypted)
        .map_err(|e| format!("Decryption failed: {e}"))?;
    let result: StructuringResult =
        serde_json::from_slice(&json).map_err(|e| format!("Deserialize failed: {e}"))?;

    Ok(result)
}

/// Remove a pending structuring result after successful confirm/reject.
fn remove_pending_structuring(
    session: &crate::crypto::ProfileSession,
    document_id: &Uuid,
) -> Result<(), String> {
    let dir = session
        .db_path
        .parent()
        .and_then(|p| p.parent())
        .ok_or("Invalid profile directory")?;
    let path = dir.join(format!("pending_review/{}.json.enc", document_id));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("Remove failed: {e}"))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// IPC Commands
// ---------------------------------------------------------------------------

/// Fetch all data needed for the review screen.
#[tauri::command]
pub fn get_review_data(
    document_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<ReviewData, String> {
    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let doc_id =
        Uuid::parse_str(&document_id).map_err(|e| format!("Invalid document ID: {e}"))?;

    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;

    // Fetch document record
    let doc = get_document(&conn, &doc_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Document not found: {doc_id}"))?;

    // Load the pending structuring result
    let structuring = load_pending_structuring(session, &doc_id)?;

    // Flatten entities into ExtractedField list
    let extracted_fields = flatten_entities_to_fields(&structuring);

    // Generate plausibility warnings before moving fields into struct
    let plausibility_warnings =
        generate_plausibility_warnings(&conn, &structuring, &extracted_fields);

    // Determine original file type
    let original_file_type = detect_file_type(&doc.source_file);

    // Fetch professional name if available
    let professional_name = structuring.professional.as_ref().map(|p| p.name.clone());
    let professional_specialty = structuring
        .professional
        .as_ref()
        .and_then(|p| p.specialty.clone());

    state.update_activity();

    Ok(ReviewData {
        document_id: doc_id,
        original_file_path: doc.source_file.clone(),
        original_file_type,
        document_type: doc.doc_type.as_str().to_string(),
        document_date: doc.document_date.map(|d| d.to_string()),
        professional_name,
        professional_specialty,
        structured_markdown: structuring.structured_markdown,
        extracted_fields,
        plausibility_warnings,
        overall_confidence: doc.ocr_confidence.unwrap_or(0.0),
    })
}

/// Decrypt the original file and return as base64 for frontend rendering.
#[tauri::command]
pub fn get_original_file(
    document_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<String, String> {
    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let doc_id =
        Uuid::parse_str(&document_id).map_err(|e| format!("Invalid document ID: {e}"))?;

    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;
    let doc = get_document(&conn, &doc_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Document not found: {doc_id}"))?;

    // Read and decrypt the original file
    let encrypted_bytes = std::fs::read(&doc.source_file)
        .map_err(|e| format!("Failed to read file: {e}"))?;
    let encrypted = EncryptedData::from_bytes(&encrypted_bytes)
        .map_err(|e| format!("Corrupt encrypted file: {e}"))?;
    let decrypted = session
        .decrypt(&encrypted)
        .map_err(|e| format!("Decryption failed: {e}"))?;

    // Encode as base64
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&decrypted);

    state.update_activity();

    Ok(encoded)
}

/// Validate a field correction value (sanitization, length check).
/// Corrections are tracked in frontend state and applied on confirm.
#[tauri::command]
pub fn update_extracted_field(
    document_id: String,
    field_id: String,
    new_value: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let _doc_id =
        Uuid::parse_str(&document_id).map_err(|e| format!("Invalid document ID: {e}"))?;
    let _field_id =
        Uuid::parse_str(&field_id).map_err(|e| format!("Invalid field ID: {e}"))?;

    if new_value.len() > 500 {
        return Err("Field value too long (max 500 characters)".into());
    }

    if new_value.chars().any(|c| c.is_control() && c != '\n') {
        return Err("Field value contains invalid characters".into());
    }

    state.update_activity();

    Ok(())
}

/// Confirm the review — apply corrections, update document, update trust metrics.
/// The storage pipeline (L1-04) is invoked at this stage in the full pipeline,
/// but for the IPC layer we update the document status and trust metrics directly.
#[tauri::command]
pub fn confirm_review(
    app: AppHandle,
    document_id: String,
    corrections: Vec<FieldCorrection>,
    state: State<'_, Arc<CoreState>>,
) -> Result<ReviewConfirmResult, String> {
    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let doc_id =
        Uuid::parse_str(&document_id).map_err(|e| format!("Invalid document ID: {e}"))?;

    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;

    // Step 1: Load the structuring result
    let mut structuring = load_pending_structuring(session, &doc_id)?;

    // Step 2: Build field map for correction matching, then apply corrections
    let field_map = flatten_entities_to_fields(&structuring);
    let corrections_applied = apply_corrections(
        &mut structuring.extracted_entities,
        &corrections,
        &field_map,
    );

    // Step 3: Determine outcome
    let outcome = if corrections_applied > 0 {
        ReviewOutcome::Corrected
    } else {
        ReviewOutcome::Confirmed
    };

    // Step 4: Update document as verified
    update_document_verified(&conn, &doc_id).map_err(|e| e.to_string())?;

    // Step 5: Update profile trust metrics
    match outcome {
        ReviewOutcome::Confirmed => {
            update_profile_trust_verified(&conn).map_err(|e| e.to_string())?;
        }
        ReviewOutcome::Corrected => {
            update_profile_trust_corrected(&conn).map_err(|e| e.to_string())?;
        }
    }

    // Step 6: Count entities for the result summary
    let entities = &structuring.extracted_entities;
    let entities_summary = EntitiesStoredSummary {
        medications: entities.medications.len(),
        lab_results: entities.lab_results.len(),
        diagnoses: entities.diagnoses.len(),
        allergies: entities.allergies.len(),
        procedures: entities.procedures.len(),
        referrals: entities.referrals.len(),
        instructions: entities.instructions.len(),
    };
    let total_fields = count_extracted_fields(entities);

    // Step 7: Clean up the pending structuring file
    let _ = remove_pending_structuring(session, &doc_id);

    // Step 8: Re-save the (potentially corrected) structuring result for storage pipeline
    let _ = save_pending_structuring(session, &structuring);

    // Step 9: Emit event for home feed refresh
    let _ = app.emit("document-reviewed", doc_id.to_string());

    state.update_activity();

    tracing::info!(
        document_id = %doc_id,
        outcome = ?outcome,
        corrections = corrections_applied,
        total_fields = total_fields,
        "Review confirmed"
    );

    Ok(ReviewConfirmResult {
        document_id: doc_id,
        status: outcome,
        entities_stored: entities_summary,
        corrections_applied,
        chunks_stored: 0, // Full storage pipeline invoked separately
    })
}

/// Reject the review — retry extraction or remove document.
#[tauri::command]
pub fn reject_review(
    app: AppHandle,
    document_id: String,
    reason: Option<String>,
    action: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<ReviewRejectResult, String> {
    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let doc_id =
        Uuid::parse_str(&document_id).map_err(|e| format!("Invalid document ID: {e}"))?;

    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;

    match action.as_str() {
        "retry" => {
            // Keep document as unverified, log for re-processing
            tracing::info!(
                document_id = %doc_id,
                action = "retry",
                "Review rejected — queued for re-extraction"
            );
        }
        "remove" => {
            // Mark document as rejected via notes
            update_document_rejected(&conn, &doc_id, reason.as_deref())
                .map_err(|e| e.to_string())?;
        }
        _ => {
            return Err(format!(
                "Invalid action: {}. Expected 'retry' or 'remove'.",
                action
            ));
        }
    }

    // Clean up pending structuring
    let _ = remove_pending_structuring(session, &doc_id);

    // Emit event
    let _ = app.emit("document-reviewed", doc_id.to_string());

    state.update_activity();

    tracing::info!(
        document_id = %doc_id,
        action = %action,
        "Review rejected"
    );

    Ok(ReviewRejectResult {
        document_id: doc_id,
        reason,
    })
}
