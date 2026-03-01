//! BTL-10 C4: Import queue IPC commands.

use std::sync::Arc;

use tauri::State;

use crate::core_state::CoreState;
use crate::import_queue::QueueSnapshot;

/// Enqueue one or more files for import. Returns job IDs.
///
/// UC-01: `document_type` bypasses LLM classification when provided.
/// Values: `"lab_report"`, `"prescription"`, `"medical_image"`.
#[tauri::command]
pub fn enqueue_imports(
    file_paths: Vec<String>,
    document_type: Option<String>,
    state: State<'_, Arc<CoreState>>,
) -> Vec<String> {
    let queue = state.import_queue();
    file_paths
        .into_iter()
        .map(|path| queue.enqueue(path, document_type.clone()))
        .collect()
}

/// Get a snapshot of the entire import queue.
#[tauri::command]
pub fn get_import_queue(
    state: State<'_, Arc<CoreState>>,
) -> QueueSnapshot {
    state.import_queue().snapshot()
}

/// Cancel an import job.
#[tauri::command]
pub fn cancel_import(
    job_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    state
        .import_queue()
        .cancel(&job_id)
        .map_err(|e| e.to_string())
}

/// Retry a failed import job. Returns the new job ID.
#[tauri::command]
pub fn retry_import(
    job_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<String, String> {
    state
        .import_queue()
        .retry(&job_id)
        .map_err(|e| e.to_string())
}

/// Delete a terminal import job from the queue.
#[tauri::command]
pub fn delete_import(
    job_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    state
        .import_queue()
        .delete(&job_id)
        .map_err(|e| e.to_string())
}
