//! BTL-10 C3: Import queue service — in-memory job lifecycle manager.
//!
//! Pure state machine with no I/O. The worker loop (C4) drives actual processing.
//! Thread-safe via Mutex. Notify wakes the worker when jobs are enqueued.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// State machine for an import job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    Queued,
    Importing,
    Extracting,
    Structuring,
    PendingReview,
    Done,
    Failed,
    Cancelled,
}

impl JobState {
    /// Whether this state is terminal (no further transitions).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }

    /// Valid next states from this state (Signal JobManager pattern: strict state machine).
    fn valid_transitions(&self) -> &'static [JobState] {
        match self {
            Self::Queued => &[Self::Importing, Self::Cancelled, Self::Failed],
            Self::Importing => &[Self::Extracting, Self::Failed, Self::Cancelled],
            Self::Extracting => &[Self::Structuring, Self::Failed, Self::Cancelled],
            Self::Structuring => &[Self::PendingReview, Self::Done, Self::Failed, Self::Cancelled],
            Self::PendingReview => &[Self::Done],
            Self::Done | Self::Cancelled => &[],
            Self::Failed => &[Self::Cancelled],  // retry auto-dismisses
        }
    }

    /// Check if transitioning to `target` is valid.
    pub fn can_transition_to(&self, target: &JobState) -> bool {
        self.valid_transitions().contains(target)
    }
}

/// A single import job in the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportJob {
    pub id: String,
    pub file_path: String,
    pub filename: String,
    pub state: JobState,
    pub progress_pct: u8,
    pub document_id: Option<String>,
    pub model_used: Option<String>,
    pub error: Option<String>,
    pub queued_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    /// UC-01: User-selected document type at import time.
    /// When Some, bypasses LLM classifier. Values: "lab_report", "prescription", "medical_image".
    pub user_document_type: Option<String>,
}

/// A snapshot of the entire queue (for IPC serialization).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueSnapshot {
    pub jobs: Vec<ImportJob>,
    pub is_running: bool,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// In-memory import queue with job lifecycle management.
pub struct ImportQueueService {
    jobs: Mutex<Vec<ImportJob>>,
    notify: tokio::sync::Notify,
    running: AtomicBool,
    /// §21 Fix C: Cooperative cancellation tokens shared with worker/processor.
    cancellation_tokens: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl ImportQueueService {
    pub fn new() -> Self {
        Self {
            jobs: Mutex::new(Vec::new()),
            notify: tokio::sync::Notify::new(),
            running: AtomicBool::new(false),
            cancellation_tokens: Mutex::new(HashMap::new()),
        }
    }

    /// Enqueue a file for import. Returns the job ID.
    ///
    /// UC-01: `user_document_type` bypasses LLM classification when provided.
    /// Values: `"lab_report"`, `"prescription"`, `"medical_image"`.
    pub fn enqueue(&self, file_path: String, user_document_type: Option<String>) -> String {
        let filename = std::path::Path::new(&file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&file_path)
            .to_string();

        let job = ImportJob {
            id: Uuid::new_v4().to_string(),
            file_path,
            filename,
            state: JobState::Queued,
            progress_pct: 0,
            document_id: None,
            model_used: None,
            error: None,
            queued_at: Utc::now().to_rfc3339(),
            started_at: None,
            completed_at: None,
            user_document_type,
        };

        let id = job.id.clone();
        let mut jobs = self.jobs.lock().expect("import queue lock poisoned");
        jobs.push(job);
        drop(jobs);

        self.notify.notify_one();
        id
    }

    /// Take the next queued job (transitions it to Importing).
    pub fn next_queued(&self) -> Option<ImportJob> {
        let mut jobs = self.jobs.lock().expect("import queue lock poisoned");
        let pos = jobs.iter().position(|j| j.state == JobState::Queued)?;
        jobs[pos].state = JobState::Importing;
        jobs[pos].started_at = Some(Utc::now().to_rfc3339());
        Some(jobs[pos].clone())
    }

    /// Update the state of a job. Returns Err if job not found or transition invalid.
    pub fn update_job_state(
        &self,
        job_id: &str,
        new_state: JobState,
        progress_pct: Option<u8>,
        document_id: Option<String>,
        model_used: Option<String>,
        error: Option<String>,
    ) -> Result<(), QueueError> {
        let mut jobs = self.jobs.lock().expect("import queue lock poisoned");
        let job = jobs.iter_mut().find(|j| j.id == job_id)
            .ok_or(QueueError::JobNotFound)?;

        if !job.state.can_transition_to(&new_state) {
            return Err(QueueError::InvalidTransition {
                from: format!("{:?}", job.state),
                to: format!("{:?}", new_state),
            });
        }

        job.state = new_state.clone();
        if let Some(pct) = progress_pct {
            job.progress_pct = pct;
        }
        if let Some(doc_id) = document_id {
            job.document_id = Some(doc_id);
        }
        if let Some(model) = model_used {
            job.model_used = Some(model);
        }
        if let Some(err) = error {
            job.error = Some(err);
        }

        if new_state.is_terminal() {
            job.completed_at = Some(Utc::now().to_rfc3339());
        }

        Ok(())
    }

    /// Cancel a job. Only Queued or active (Importing/Extracting/Structuring) jobs can be cancelled.
    ///
    /// §21 Fix C: Also signals the cancellation token (if one exists) so the
    /// processor can detect cancellation at its next checkpoint.
    pub fn cancel(&self, job_id: &str) -> Result<(), QueueError> {
        let mut jobs = self.jobs.lock().expect("import queue lock poisoned");
        let job = jobs.iter_mut().find(|j| j.id == job_id)
            .ok_or(QueueError::JobNotFound)?;

        match job.state {
            JobState::Queued | JobState::Importing | JobState::Extracting | JobState::Structuring => {
                job.state = JobState::Cancelled;
                job.completed_at = Some(Utc::now().to_rfc3339());

                // Signal the processor to stop at its next checkpoint
                let tokens = self.cancellation_tokens.lock()
                    .expect("cancellation tokens lock poisoned");
                if let Some(token) = tokens.get(job_id) {
                    token.store(true, Ordering::Relaxed);
                }

                Ok(())
            }
            _ => Err(QueueError::InvalidTransition {
                from: format!("{:?}", job.state),
                to: "Cancelled".into(),
            }),
        }
    }

    /// Create a cancellation token for a job. Called by worker before spawn_blocking.
    /// Returns a shared `Arc<AtomicBool>` that the processor checks at cancellation checkpoints.
    pub fn create_cancellation_token(&self, job_id: &str) -> Arc<AtomicBool> {
        let token = Arc::new(AtomicBool::new(false));
        let mut tokens = self.cancellation_tokens.lock()
            .expect("cancellation tokens lock poisoned");
        tokens.insert(job_id.to_string(), Arc::clone(&token));
        token
    }

    /// Remove a cancellation token after job completes (success, failure, or cancel).
    pub fn remove_cancellation_token(&self, job_id: &str) {
        let mut tokens = self.cancellation_tokens.lock()
            .expect("cancellation tokens lock poisoned");
        tokens.remove(job_id);
    }

    /// Update job progress and/or metadata without requiring a state transition.
    ///
    /// Used by StageWatcher for incremental progress within a stage,
    /// and by the worker to set `model_used` before processing begins.
    /// Rejects updates to terminal-state jobs.
    pub fn update_job_progress(
        &self,
        job_id: &str,
        progress_pct: Option<u8>,
        model_used: Option<String>,
    ) -> Result<(), QueueError> {
        let mut jobs = self.jobs.lock().expect("import queue lock poisoned");
        let job = jobs.iter_mut().find(|j| j.id == job_id)
            .ok_or(QueueError::JobNotFound)?;

        if job.state.is_terminal() {
            return Err(QueueError::InvalidTransition {
                from: format!("{:?}", job.state),
                to: "progress update".into(),
            });
        }

        if let Some(pct) = progress_pct {
            job.progress_pct = pct;
        }
        if let Some(model) = model_used {
            job.model_used = Some(model);
        }
        Ok(())
    }

    /// Retry a failed job. Marks old job as Cancelled (auto-dismissed from UI),
    /// then creates a new Queued entry with the same file. Returns the new job ID.
    ///
    /// UC-01: Preserves `user_document_type` from the original job.
    pub fn retry(&self, job_id: &str) -> Result<String, QueueError> {
        let (file_path, user_document_type) = {
            let mut jobs = self.jobs.lock().expect("import queue lock poisoned");
            let job = jobs.iter_mut().find(|j| j.id == job_id)
                .ok_or(QueueError::JobNotFound)?;

            if job.state != JobState::Failed {
                return Err(QueueError::InvalidTransition {
                    from: format!("{:?}", job.state),
                    to: "Queued (retry)".into(),
                });
            }

            let file_path = job.file_path.clone();
            let user_document_type = job.user_document_type.clone();
            // Auto-dismiss: mark old failed job as Cancelled (filtered from visibleItems)
            job.state = JobState::Cancelled;
            job.completed_at = Some(Utc::now().to_rfc3339());
            (file_path, user_document_type)
        };

        Ok(self.enqueue(file_path, user_document_type))
    }

    /// Delete a terminal job from the queue.
    pub fn delete(&self, job_id: &str) -> Result<(), QueueError> {
        let mut jobs = self.jobs.lock().expect("import queue lock poisoned");
        let pos = jobs.iter().position(|j| j.id == job_id)
            .ok_or(QueueError::JobNotFound)?;

        if !jobs[pos].state.is_terminal() {
            return Err(QueueError::InvalidTransition {
                from: format!("{:?}", jobs[pos].state),
                to: "Deleted".into(),
            });
        }

        jobs.remove(pos);
        Ok(())
    }

    /// Get a snapshot of all jobs.
    pub fn snapshot(&self) -> QueueSnapshot {
        let jobs = self.jobs.lock().expect("import queue lock poisoned");
        QueueSnapshot {
            jobs: jobs.clone(),
            is_running: self.running.load(Ordering::Relaxed),
        }
    }

    /// Get a reference to the Notify handle (for the worker to await).
    pub fn notifier(&self) -> &tokio::sync::Notify {
        &self.notify
    }

    /// Set whether the worker is actively processing.
    pub fn set_running(&self, running: bool) {
        self.running.store(running, Ordering::Relaxed);
    }

    /// Check if the worker is actively processing.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Reset the queue (for profile switch — F7 security).
    pub fn reset(&self) {
        let mut jobs = self.jobs.lock().expect("import queue lock poisoned");
        jobs.clear();
        self.running.store(false, Ordering::Relaxed);
        // §21 Fix C: Clear cancellation tokens on profile switch
        let mut tokens = self.cancellation_tokens.lock()
            .expect("cancellation tokens lock poisoned");
        tokens.clear();
    }

    /// Get a single job by ID.
    pub fn get_job(&self, job_id: &str) -> Option<ImportJob> {
        let jobs = self.jobs.lock().expect("import queue lock poisoned");
        jobs.iter().find(|j| j.id == job_id).cloned()
    }

    /// Count of non-terminal jobs.
    pub fn active_count(&self) -> usize {
        let jobs = self.jobs.lock().expect("import queue lock poisoned");
        jobs.iter().filter(|j| !j.state.is_terminal()).count()
    }

    /// C11: Recover interrupted imports from DB on app restart.
    ///
    /// Queries documents with non-terminal pipeline_status (Imported, Extracting,
    /// Structuring), resets them to Imported, and enqueues them as Queued jobs.
    /// Called during profile hydration after `open_db()`.
    pub fn recover_from_db(
        &self,
        conn: &rusqlite::Connection,
    ) -> Result<usize, String> {
        use crate::db::repository;
        use crate::models::enums::PipelineStatus;

        let interrupted_statuses = [
            PipelineStatus::Imported,
            PipelineStatus::Extracting,
            PipelineStatus::Structuring,
        ];

        let mut recovered = 0;

        for status in &interrupted_statuses {
            let docs = repository::get_documents_by_pipeline_status(conn, status)
                .map_err(|e| e.to_string())?;

            for doc in docs {
                // Reset to Imported so the pipeline can start fresh.
                if *status != PipelineStatus::Imported {
                    repository::update_pipeline_status(
                        conn,
                        &doc.id,
                        &PipelineStatus::Imported,
                    )
                    .map_err(|e| e.to_string())?;
                }

                // Enqueue as a Queued job.
                let filename = std::path::Path::new(&doc.source_file)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&doc.source_file)
                    .to_string();

                let job = ImportJob {
                    id: Uuid::new_v4().to_string(),
                    file_path: doc.source_file.clone(),
                    filename,
                    state: JobState::Queued,
                    progress_pct: 0,
                    document_id: Some(doc.id.to_string()),
                    model_used: None,
                    error: None,
                    queued_at: Utc::now().to_rfc3339(),
                    started_at: None,
                    completed_at: None,
                    user_document_type: None, // Recovery: fallback to LLM classifier
                };

                let mut jobs = self.jobs.lock().expect("import queue lock poisoned");
                jobs.push(job);
                recovered += 1;
            }
        }

        if recovered > 0 {
            tracing::info!(recovered, "Recovered interrupted imports from DB");
            self.notify.notify_one();
        }

        Ok(recovered)
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueError {
    JobNotFound,
    InvalidTransition { from: String, to: String },
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JobNotFound => write!(f, "Job not found in queue"),
            Self::InvalidTransition { from, to } => {
                write!(f, "Invalid state transition: {from} → {to}")
            }
        }
    }
}

impl std::error::Error for QueueError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> ImportQueueService {
        ImportQueueService::new()
    }

    // -- Enqueue --

    #[test]
    fn enqueue_creates_queued_job() {
        let svc = service();
        let id = svc.enqueue("/tmp/scan.pdf".into(), None);

        let snap = svc.snapshot();
        assert_eq!(snap.jobs.len(), 1);
        assert_eq!(snap.jobs[0].id, id);
        assert_eq!(snap.jobs[0].state, JobState::Queued);
        assert_eq!(snap.jobs[0].filename, "scan.pdf");
        assert_eq!(snap.jobs[0].progress_pct, 0);
    }

    #[test]
    fn enqueue_preserves_order() {
        let svc = service();
        let id1 = svc.enqueue("/tmp/a.pdf".into(), None);
        let id2 = svc.enqueue("/tmp/b.pdf".into(), None);
        let id3 = svc.enqueue("/tmp/c.pdf".into(), None);

        let snap = svc.snapshot();
        assert_eq!(snap.jobs[0].id, id1);
        assert_eq!(snap.jobs[1].id, id2);
        assert_eq!(snap.jobs[2].id, id3);
    }

    // -- next_queued --

    #[test]
    fn next_queued_transitions_to_importing() {
        let svc = service();
        svc.enqueue("/tmp/a.pdf".into(), None);

        let job = svc.next_queued().unwrap();
        assert_eq!(job.state, JobState::Importing);
        assert!(job.started_at.is_some());

        // Should not find another queued job
        assert!(svc.next_queued().is_none());
    }

    #[test]
    fn next_queued_skips_non_queued() {
        let svc = service();
        let id1 = svc.enqueue("/tmp/a.pdf".into(), None);
        svc.enqueue("/tmp/b.pdf".into(), None);

        // Take first
        svc.next_queued().unwrap();
        // Cancel first
        svc.cancel(&id1).unwrap();

        // Second should be next
        let job = svc.next_queued().unwrap();
        assert_eq!(job.filename, "b.pdf");
    }

    #[test]
    fn next_queued_empty_returns_none() {
        let svc = service();
        assert!(svc.next_queued().is_none());
    }

    // -- update_job_state --

    #[test]
    fn update_job_state_transitions() {
        let svc = service();
        let id = svc.enqueue("/tmp/a.pdf".into(), None);
        svc.next_queued();

        svc.update_job_state(&id, JobState::Extracting, Some(25), None, Some("medgemma-4b".into()), None).unwrap();
        let job = svc.get_job(&id).unwrap();
        assert_eq!(job.state, JobState::Extracting);
        assert_eq!(job.progress_pct, 25);
        assert_eq!(job.model_used.as_deref(), Some("medgemma-4b"));

        svc.update_job_state(&id, JobState::Structuring, Some(60), Some("doc-123".into()), None, None).unwrap();
        let job = svc.get_job(&id).unwrap();
        assert_eq!(job.state, JobState::Structuring);
        assert_eq!(job.document_id.as_deref(), Some("doc-123"));

        svc.update_job_state(&id, JobState::Done, Some(100), None, None, None).unwrap();
        let job = svc.get_job(&id).unwrap();
        assert_eq!(job.state, JobState::Done);
        assert!(job.completed_at.is_some());
    }

    #[test]
    fn update_job_state_failed_with_error() {
        let svc = service();
        let id = svc.enqueue("/tmp/a.pdf".into(), None);
        svc.next_queued();

        svc.update_job_state(&id, JobState::Failed, None, None, None, Some("OCR timeout".into())).unwrap();
        let job = svc.get_job(&id).unwrap();
        assert_eq!(job.state, JobState::Failed);
        assert_eq!(job.error.as_deref(), Some("OCR timeout"));
        assert!(job.completed_at.is_some());
    }

    #[test]
    fn update_nonexistent_returns_error() {
        let svc = service();
        assert_eq!(
            svc.update_job_state("nonexistent", JobState::Done, None, None, None, None).unwrap_err(),
            QueueError::JobNotFound,
        );
    }

    #[test]
    fn update_invalid_transition_rejected() {
        let svc = service();
        let id = svc.enqueue("/tmp/a.pdf".into(), None);
        // Queued → Done is invalid (must go through Importing → Extracting → Structuring)
        let err = svc.update_job_state(&id, JobState::Done, None, None, None, None).unwrap_err();
        assert!(matches!(err, QueueError::InvalidTransition { .. }));

        // Done → Queued (terminal to non-terminal) is also invalid
        svc.next_queued();
        svc.update_job_state(&id, JobState::Extracting, None, None, None, None).unwrap();
        svc.update_job_state(&id, JobState::Structuring, None, None, None, None).unwrap();
        svc.update_job_state(&id, JobState::Done, None, None, None, None).unwrap();
        let err = svc.update_job_state(&id, JobState::Queued, None, None, None, None).unwrap_err();
        assert!(matches!(err, QueueError::InvalidTransition { .. }));
    }

    // -- cancel --

    #[test]
    fn cancel_queued_job() {
        let svc = service();
        let id = svc.enqueue("/tmp/a.pdf".into(), None);
        svc.cancel(&id).unwrap();

        let job = svc.get_job(&id).unwrap();
        assert_eq!(job.state, JobState::Cancelled);
        assert!(job.completed_at.is_some());
    }

    #[test]
    fn cancel_active_job() {
        let svc = service();
        let id = svc.enqueue("/tmp/a.pdf".into(), None);
        svc.next_queued();
        svc.update_job_state(&id, JobState::Extracting, Some(50), None, None, None).unwrap();

        svc.cancel(&id).unwrap();
        assert_eq!(svc.get_job(&id).unwrap().state, JobState::Cancelled);
    }

    #[test]
    fn cancel_terminal_fails() {
        let svc = service();
        let id = svc.enqueue("/tmp/a.pdf".into(), None);
        svc.next_queued();
        svc.update_job_state(&id, JobState::Extracting, None, None, None, None).unwrap();
        svc.update_job_state(&id, JobState::Structuring, None, None, None, None).unwrap();
        svc.update_job_state(&id, JobState::Done, Some(100), None, None, None).unwrap();

        let err = svc.cancel(&id).unwrap_err();
        assert_eq!(err, QueueError::InvalidTransition {
            from: "Done".into(),
            to: "Cancelled".into(),
        });
    }

    #[test]
    fn cancel_nonexistent_fails() {
        let svc = service();
        assert_eq!(svc.cancel("nonexistent"), Err(QueueError::JobNotFound));
    }

    // -- retry --

    #[test]
    fn retry_failed_job() {
        let svc = service();
        let id = svc.enqueue("/tmp/a.pdf".into(), None);
        svc.next_queued();
        svc.update_job_state(&id, JobState::Failed, None, None, None, Some("error".into())).unwrap();

        let new_id = svc.retry(&id).unwrap();
        assert_ne!(id, new_id);

        let snap = svc.snapshot();
        assert_eq!(snap.jobs.len(), 2);

        // Old job auto-dismissed to Cancelled
        let old_job = snap.jobs.iter().find(|j| j.id == id).unwrap();
        assert_eq!(old_job.state, JobState::Cancelled);
        assert!(old_job.completed_at.is_some());

        // New job is Queued with same file
        let new_job = snap.jobs.iter().find(|j| j.id == new_id).unwrap();
        assert_eq!(new_job.state, JobState::Queued);
        assert_eq!(new_job.file_path, "/tmp/a.pdf");
    }

    #[test]
    fn retry_non_failed_fails() {
        let svc = service();
        let id = svc.enqueue("/tmp/a.pdf".into(), None);

        let err = svc.retry(&id).unwrap_err();
        assert_eq!(err, QueueError::InvalidTransition {
            from: "Queued".into(),
            to: "Queued (retry)".into(),
        });
    }

    #[test]
    fn retry_old_job_excluded_from_active_count() {
        let svc = service();
        let id = svc.enqueue("/tmp/a.pdf".into(), None);
        svc.next_queued();
        svc.update_job_state(&id, JobState::Failed, None, None, None, Some("err".into())).unwrap();
        svc.retry(&id).unwrap();
        // Only the new Queued job should be active
        assert_eq!(svc.active_count(), 1);
    }

    /// 12-ERC race: StageWatcher misses near-instant Structuring → queue stuck at Extracting.
    /// Direct PendingReview transition is invalid, must catch up through Structuring first.
    #[test]
    fn extracting_to_pending_review_requires_catchup() {
        let svc = service();
        let id = svc.enqueue("/tmp/a.pdf".into(), None);
        svc.next_queued();
        svc.update_job_state(&id, JobState::Extracting, Some(30), None, None, None).unwrap();

        // Direct Extracting → PendingReview is invalid (the race condition bug)
        let err = svc.update_job_state(&id, JobState::PendingReview, Some(95), None, None, None)
            .unwrap_err();
        assert!(matches!(err, QueueError::InvalidTransition { .. }));

        // Catch-up: Extracting → Structuring → PendingReview → Done (the fix)
        svc.update_job_state(&id, JobState::Structuring, Some(50), None, None, None).unwrap();
        svc.update_job_state(&id, JobState::PendingReview, Some(95), None, None, None).unwrap();
        svc.update_job_state(&id, JobState::Done, Some(100), None, None, None).unwrap();
        assert_eq!(svc.get_job(&id).unwrap().state, JobState::Done);
    }

    #[test]
    fn failed_to_cancelled_is_valid_transition() {
        assert!(JobState::Failed.can_transition_to(&JobState::Cancelled));
        // Terminal states remain terminal
        assert!(!JobState::Done.can_transition_to(&JobState::Cancelled));
        assert!(!JobState::Cancelled.can_transition_to(&JobState::Failed));
    }

    // -- delete --

    #[test]
    fn delete_terminal_job() {
        let svc = service();
        let id = svc.enqueue("/tmp/a.pdf".into(), None);
        svc.next_queued();
        svc.update_job_state(&id, JobState::Extracting, None, None, None, None).unwrap();
        svc.update_job_state(&id, JobState::Structuring, None, None, None, None).unwrap();
        svc.update_job_state(&id, JobState::Done, Some(100), None, None, None).unwrap();

        svc.delete(&id).unwrap();
        assert_eq!(svc.snapshot().jobs.len(), 0);
    }

    #[test]
    fn delete_active_fails() {
        let svc = service();
        let id = svc.enqueue("/tmp/a.pdf".into(), None);

        let err = svc.delete(&id).unwrap_err();
        assert_eq!(err, QueueError::InvalidTransition {
            from: "Queued".into(),
            to: "Deleted".into(),
        });
    }

    // -- snapshot --

    #[test]
    fn snapshot_reflects_state() {
        let svc = service();
        svc.enqueue("/tmp/a.pdf".into(), None);
        svc.enqueue("/tmp/b.pdf".into(), None);

        let snap = svc.snapshot();
        assert_eq!(snap.jobs.len(), 2);
        assert!(!snap.is_running);
    }

    // -- reset --

    #[test]
    fn reset_clears_all() {
        let svc = service();
        svc.enqueue("/tmp/a.pdf".into(), None);
        svc.enqueue("/tmp/b.pdf".into(), None);
        svc.set_running(true);

        svc.reset();

        let snap = svc.snapshot();
        assert!(snap.jobs.is_empty());
        assert!(!snap.is_running);
    }

    // -- active_count --

    #[test]
    fn active_count_excludes_terminal() {
        let svc = service();
        let id1 = svc.enqueue("/tmp/a.pdf".into(), None);
        svc.enqueue("/tmp/b.pdf".into(), None);

        assert_eq!(svc.active_count(), 2);

        svc.next_queued();
        svc.update_job_state(&id1, JobState::Extracting, None, None, None, None).unwrap();
        svc.update_job_state(&id1, JobState::Structuring, None, None, None, None).unwrap();
        svc.update_job_state(&id1, JobState::Done, Some(100), None, None, None).unwrap();

        assert_eq!(svc.active_count(), 1);
    }

    // -- concurrent access --

    #[test]
    fn concurrent_enqueue() {
        let svc = std::sync::Arc::new(service());
        let mut handles = Vec::new();

        for i in 0..10 {
            let svc = svc.clone();
            handles.push(std::thread::spawn(move || {
                svc.enqueue(format!("/tmp/file_{i}.pdf"), None);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(svc.snapshot().jobs.len(), 10);
    }

    // -- Recovery from DB (C11) --

    mod recovery {
        use super::*;
        use crate::db::sqlite::open_memory_database;
        use crate::db::repository;
        use crate::models::*;
        use crate::models::enums::*;

        fn insert_doc(conn: &rusqlite::Connection, file: &str, status: PipelineStatus) -> Uuid {
            let id = Uuid::new_v4();
            repository::insert_document(
                conn,
                &Document {
                    id,
                    doc_type: DocumentType::Prescription,
                    title: "Test".into(),
                    document_date: None,
                    ingestion_date: chrono::Local::now().naive_local(),
                    professional_id: None,
                    source_file: file.into(),
                    markdown_file: None,
                    ocr_confidence: Some(0.9),
                    verified: false,
                    source_deleted: false,
                    perceptual_hash: None,
                    notes: None,
                    pipeline_status: status,
                },
            )
            .unwrap();
            id
        }

        #[test]
        fn recover_empty_db_yields_zero() {
            let conn = open_memory_database().unwrap();
            let svc = service();
            let count = svc.recover_from_db(&conn).unwrap();
            assert_eq!(count, 0);
            assert_eq!(svc.snapshot().jobs.len(), 0);
        }

        #[test]
        fn recover_finds_interrupted_docs() {
            let conn = open_memory_database().unwrap();
            insert_doc(&conn, "/tmp/a.pdf", PipelineStatus::Imported);
            insert_doc(&conn, "/tmp/b.pdf", PipelineStatus::Extracting);
            insert_doc(&conn, "/tmp/c.pdf", PipelineStatus::Structuring);
            // Terminal statuses should NOT be recovered:
            insert_doc(&conn, "/tmp/d.pdf", PipelineStatus::PendingReview);
            insert_doc(&conn, "/tmp/e.pdf", PipelineStatus::Confirmed);
            insert_doc(&conn, "/tmp/f.pdf", PipelineStatus::Failed);

            let svc = service();
            let count = svc.recover_from_db(&conn).unwrap();
            assert_eq!(count, 3);

            let snap = svc.snapshot();
            assert_eq!(snap.jobs.len(), 3);
            // All should be Queued
            assert!(snap.jobs.iter().all(|j| j.state == JobState::Queued));
        }

        #[test]
        fn recover_resets_status_to_imported() {
            let conn = open_memory_database().unwrap();
            let doc_id = insert_doc(&conn, "/tmp/stuck.pdf", PipelineStatus::Extracting);

            let svc = service();
            svc.recover_from_db(&conn).unwrap();

            // DB status should be reset to Imported
            let doc = repository::get_document(&conn, &doc_id).unwrap().unwrap();
            assert_eq!(doc.pipeline_status, PipelineStatus::Imported);
        }

        #[test]
        fn recover_preserves_document_id() {
            let conn = open_memory_database().unwrap();
            let doc_id = insert_doc(&conn, "/tmp/resume.pdf", PipelineStatus::Imported);

            let svc = service();
            svc.recover_from_db(&conn).unwrap();

            let snap = svc.snapshot();
            assert_eq!(snap.jobs.len(), 1);
            assert_eq!(snap.jobs[0].document_id.as_deref(), Some(doc_id.to_string().as_str()));
        }

        #[test]
        fn recover_extracts_filename() {
            let conn = open_memory_database().unwrap();
            insert_doc(&conn, "/path/to/ordonnance.pdf", PipelineStatus::Imported);

            let svc = service();
            svc.recover_from_db(&conn).unwrap();

            assert_eq!(svc.snapshot().jobs[0].filename, "ordonnance.pdf");
        }
    }

    // -- Update job progress (§21 Fix B) --

    #[test]
    fn update_job_progress_updates_pct() {
        let svc = service();
        let id = svc.enqueue("/tmp/scan.pdf".into(), None);
        svc.next_queued(); // → Importing

        svc.update_job_progress(&id, Some(42), None).unwrap();

        let job = svc.get_job(&id).unwrap();
        assert_eq!(job.progress_pct, 42);
        assert_eq!(job.state, JobState::Importing); // state unchanged
    }

    #[test]
    fn update_job_progress_sets_model() {
        let svc = service();
        let id = svc.enqueue("/tmp/scan.pdf".into(), None);
        svc.next_queued();

        svc.update_job_progress(&id, None, Some("medgemma-4b".into())).unwrap();

        let job = svc.get_job(&id).unwrap();
        assert_eq!(job.model_used.as_deref(), Some("medgemma-4b"));
        assert_eq!(job.progress_pct, 0); // unchanged
    }

    #[test]
    fn update_job_progress_rejects_terminal() {
        let svc = service();
        let id = svc.enqueue("/tmp/scan.pdf".into(), None);
        svc.next_queued();
        svc.update_job_state(&id, JobState::Failed, None, None, None, Some("err".into())).unwrap();

        let result = svc.update_job_progress(&id, Some(50), None);
        assert!(result.is_err());
    }

    // -- Cancellation tokens (§21 Fix C) --

    #[test]
    fn create_cancellation_token_returns_shared_arc() {
        let svc = service();
        let id = svc.enqueue("/tmp/scan.pdf".into(), None);
        let token = svc.create_cancellation_token(&id);

        assert!(!token.load(Ordering::Relaxed));
    }

    #[test]
    fn cancel_signals_cancellation_token() {
        let svc = service();
        let id = svc.enqueue("/tmp/scan.pdf".into(), None);
        svc.next_queued(); // → Importing
        let token = svc.create_cancellation_token(&id);

        svc.cancel(&id).unwrap();

        assert!(token.load(Ordering::Relaxed));
        assert_eq!(svc.get_job(&id).unwrap().state, JobState::Cancelled);
    }

    #[test]
    fn remove_cancellation_token_cleans_up() {
        let svc = service();
        let id = svc.enqueue("/tmp/scan.pdf".into(), None);
        let token = svc.create_cancellation_token(&id);

        svc.remove_cancellation_token(&id);

        // Token still works (Arc is held), but queue no longer references it
        assert!(!token.load(Ordering::Relaxed));
        // Cancelling now won't set the token (it's been removed from the map)
        svc.next_queued();
        svc.cancel(&id).unwrap();
        assert!(!token.load(Ordering::Relaxed));
    }

    #[test]
    fn cancel_without_token_still_works() {
        let svc = service();
        let id = svc.enqueue("/tmp/scan.pdf".into(), None);
        // No cancellation token created — cancel should still work
        svc.cancel(&id).unwrap();
        assert_eq!(svc.get_job(&id).unwrap().state, JobState::Cancelled);
    }

    // -- UC-01: User document type --

    #[test]
    fn enqueue_with_document_type() {
        let svc = service();
        let id = svc.enqueue("/tmp/lab.pdf".into(), Some("lab_report".into()));

        let job = svc.get_job(&id).unwrap();
        assert_eq!(job.user_document_type.as_deref(), Some("lab_report"));
    }

    #[test]
    fn enqueue_without_document_type() {
        let svc = service();
        let id = svc.enqueue("/tmp/scan.pdf".into(), None);

        let job = svc.get_job(&id).unwrap();
        assert!(job.user_document_type.is_none());
    }

    #[test]
    fn retry_preserves_document_type() {
        let svc = service();
        let id = svc.enqueue("/tmp/lab.pdf".into(), Some("prescription".into()));
        svc.next_queued();
        svc.update_job_state(&id, JobState::Failed, None, None, None, Some("error".into())).unwrap();

        let new_id = svc.retry(&id).unwrap();
        let new_job = svc.get_job(&new_id).unwrap();
        assert_eq!(new_job.user_document_type.as_deref(), Some("prescription"));
    }

    #[test]
    fn snapshot_includes_document_type() {
        let svc = service();
        svc.enqueue("/tmp/a.pdf".into(), Some("medical_image".into()));

        let snap = svc.snapshot();
        assert_eq!(snap.jobs[0].user_document_type.as_deref(), Some("medical_image"));
    }
}
