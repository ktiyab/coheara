//! IPC commands for the Night Batch Extraction Pipeline (LP-01).
//!
//! Seven commands for the frontend:
//! 1. get_pending_extractions — fetch items awaiting review
//! 2. get_pending_extraction_count — badge count for Home screen
//! 3. confirm_extraction — confirm and dispatch one item
//! 4. confirm_extraction_with_edits — confirm with field overrides
//! 5. dismiss_extraction — dismiss one item
//! 6. dismiss_all_extractions — dismiss multiple items
//! 7. trigger_extraction_batch — manual batch trigger

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use crate::core_state::CoreState;
use crate::pipeline::batch_extraction::{
    dispatch::dispatch_item,
    store::SqlitePendingStore,
    traits::PendingReviewStore,
    types::*,
    scheduler::SqliteBatchScheduler,
    runner::{BatchRunner, run_full_batch},
    analyzer::RuleBasedAnalyzer,
    extractors::{SymptomExtractor, MedicationExtractor, AppointmentExtractor},
};

/// Fetch all pending extraction items for the morning review.
#[tauri::command]
pub fn get_pending_extractions(
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<PendingReviewItem>, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let store = SqlitePendingStore;

    let items = store
        .get_pending(&conn)
        .map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(items)
}

/// Get the count of pending extraction items (for badge/indicator).
#[tauri::command]
pub fn get_pending_extraction_count(
    state: State<'_, Arc<CoreState>>,
) -> Result<u32, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let store = SqlitePendingStore;

    let count = store
        .get_pending_count(&conn)
        .map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(count)
}

/// Confirm a single extraction item: dispatch to domain table and mark confirmed.
#[tauri::command]
pub fn confirm_extraction(
    item_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<DispatchResult, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let store = SqlitePendingStore;

    // Load the full pending item
    let item = store
        .get_item_by_id(&conn, &item_id)
        .map_err(|e| e.to_string())?;

    // Dispatch to domain table
    let result = dispatch_item(&conn, &item).map_err(|e| e.to_string())?;

    // Mark as confirmed in extraction_pending
    store
        .confirm_item(&conn, &item_id)
        .map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(result)
}

/// Confirm a single extraction item with user edits applied.
#[tauri::command]
pub fn confirm_extraction_with_edits(
    item_id: String,
    edits: serde_json::Value,
    state: State<'_, Arc<CoreState>>,
) -> Result<DispatchResult, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let store = SqlitePendingStore;

    // Load the full pending item
    let mut item = store
        .get_item_by_id(&conn, &item_id)
        .map_err(|e| e.to_string())?;

    // Merge edits into extracted_data
    if let Some(edit_obj) = edits.as_object() {
        if let Some(data_obj) = item.extracted_data.as_object_mut() {
            for (key, value) in edit_obj {
                data_obj.insert(key.clone(), value.clone());
            }
        }
    }

    // Dispatch to domain table with merged data
    let result = dispatch_item(&conn, &item).map_err(|e| e.to_string())?;

    // Mark as confirmed with edits (stores the merged data)
    store
        .confirm_item_with_edits(&conn, &item_id, item.extracted_data)
        .map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(result)
}

/// Dismiss a single extraction item.
#[tauri::command]
pub fn dismiss_extraction(
    item_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let store = SqlitePendingStore;

    store
        .dismiss_item(&conn, &item_id)
        .map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(())
}

/// Dismiss multiple extraction items at once.
#[tauri::command]
pub fn dismiss_all_extractions(
    item_ids: Vec<String>,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let store = SqlitePendingStore;

    store
        .dismiss_items(&conn, &item_ids)
        .map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(())
}

/// Manually trigger a batch extraction run.
///
/// Emits progress events via `extraction-progress` channel.
/// Runs on a blocking thread to avoid freezing the UI (sequential LLM calls).
#[tauri::command]
pub async fn trigger_extraction_batch(
    app: AppHandle,
    state: State<'_, Arc<CoreState>>,
) -> Result<BatchResult, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = state.open_db().map_err(|e| e.to_string())?;

        // Resolve the active model name
        let model_name = resolve_model_name(&state)?;

        let config = ExtractionConfig {
            model_name,
            ..ExtractionConfig::default()
        };

        let scheduler = SqliteBatchScheduler::new();
        let store = SqlitePendingStore;
        let runner = BatchRunner::new(
            Box::new(RuleBasedAnalyzer::new()),
            vec![
                Box::new(SymptomExtractor::new()),
                Box::new(MedicationExtractor::new()),
                Box::new(AppointmentExtractor::new()),
            ],
            config.clone(),
        );

        // Create the LLM client
        let llm = crate::pipeline::structuring::ollama::OllamaClient::default_local();

        let progress_fn = |event: BatchStatusEvent| {
            let _ = app.emit("extraction-progress", &event);
        };

        // Load patient context from DB for LLM disambiguation
        let patient_context = crate::pipeline::batch_extraction::context::load_patient_context(&conn)
            .unwrap_or_default();

        let result = run_full_batch(
            &conn,
            &scheduler,
            &runner,
            &store,
            &llm,
            &config,
            &patient_context,
            Some(&progress_fn),
        )
        .map_err(|e| e.to_string())?;

        state.update_activity();
        Ok(result)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

/// Resolve the active model name from preferences.
fn resolve_model_name(state: &Arc<CoreState>) -> Result<String, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let client = crate::pipeline::structuring::ollama::OllamaClient::default_local();

    let model = state
        .resolver()
        .resolve(&conn, &client)
        .ok()
        .map(|m| m.name)
        .unwrap_or_else(|| "medgemma:4b".to_string());

    Ok(model)
}
