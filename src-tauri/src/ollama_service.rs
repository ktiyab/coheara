//! Centralized Ollama service — single point of access for all SLM operations.
//!
//! **Why this exists**: Ollama serves one model at a time. Concurrent requests
//! from different commands (import, chat, verification, batch extraction) cause
//! model swapping, degrading performance for all operations. This service
//! enforces exclusive access and tracks what's running.
//!
//! **Design**:
//! - `OllamaService` lives in `CoreState` (shared via `Arc`)
//! - `acquire()` blocks until Ollama is free (for real operations)
//! - `try_acquire()` skips if busy (for verification — a running operation IS verification)
//! - `current_operation()` provides observability (what model, what kind, when started)
//! - `client()` is the single factory for properly-configured `OllamaClient` instances

use std::sync::{Mutex, MutexGuard};

use serde::Serialize;

use crate::pipeline::structuring::ollama::OllamaClient;

// ═══════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════

/// What kind of Ollama operation is running.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    /// Vision OCR extraction from document pages (DeepSeek-OCR / MedGemma)
    DocumentOcr,
    /// LLM text → structured entities extraction (MedGemma)
    DocumentStructuring,
    /// Interactive chat generation (streaming)
    ChatGeneration,
    /// Background batch extraction from conversations
    BatchExtraction,
    /// Health/capability verification (test generation)
    ModelVerification,
}

impl std::fmt::Display for OperationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DocumentOcr => write!(f, "Document OCR"),
            Self::DocumentStructuring => write!(f, "Document structuring"),
            Self::ChatGeneration => write!(f, "Chat generation"),
            Self::BatchExtraction => write!(f, "Batch extraction"),
            Self::ModelVerification => write!(f, "Model verification"),
        }
    }
}

/// Snapshot of the currently running Ollama operation.
#[derive(Debug, Clone, Serialize)]
pub struct ActiveOperation {
    /// What kind of operation is running.
    pub kind: OperationKind,
    /// Which model is being used.
    pub model: String,
    /// When the operation started (ISO 8601).
    pub started_at: String,
}

// ═══════════════════════════════════════════════════════════
// OllamaService
// ═══════════════════════════════════════════════════════════

/// Centralized Ollama access controller.
///
/// Ensures only one inference operation runs at a time and provides
/// observability into what's happening. All commands that use Ollama
/// must go through this service.
pub struct OllamaService {
    /// Exclusive access lock — only one operation at a time.
    lock: Mutex<()>,
    /// What's currently running (observable state).
    current_op: Mutex<Option<ActiveOperation>>,
}

/// Errors from OllamaService operations.
#[derive(Debug, thiserror::Error)]
pub enum OllamaServiceError {
    #[error("Internal lock error")]
    LockPoisoned,
}

impl OllamaService {
    /// Create a new OllamaService.
    pub fn new() -> Self {
        Self {
            lock: Mutex::new(()),
            current_op: Mutex::new(None),
        }
    }

    /// Create a properly-configured `OllamaClient`.
    ///
    /// All Ollama access should use this factory instead of
    /// `OllamaClient::default_local()` or `OllamaClient::from_env()`.
    /// Ensures consistent configuration (connect_timeout, no request timeouts).
    pub fn client() -> OllamaClient {
        OllamaClient::from_env()
    }

    /// Acquire exclusive access to Ollama. Blocks until available.
    ///
    /// Use for real operations (document processing, chat, batch extraction).
    /// The guard must be held for the entire operation — dropping it releases
    /// the lock and clears the current operation state.
    ///
    /// # Example
    /// ```ignore
    /// let _guard = state.ollama().acquire(OperationKind::DocumentOcr, "deepseek-ocr:latest")?;
    /// let client = OllamaService::client();
    /// // ... run pipeline ... guard dropped here
    /// ```
    pub fn acquire(
        &self,
        kind: OperationKind,
        model: &str,
    ) -> Result<OllamaGuard<'_>, OllamaServiceError> {
        let guard = self.lock.lock().map_err(|_| OllamaServiceError::LockPoisoned)?;
        self.set_current_op(kind, model);
        Ok(OllamaGuard {
            _guard: guard,
            service: self,
        })
    }

    /// Try to acquire exclusive access without blocking.
    ///
    /// Returns `None` if Ollama is busy with another operation.
    /// Use for verification — if Ollama is already running an operation,
    /// that operation IS the verification (no need to send a test prompt).
    pub fn try_acquire(
        &self,
        kind: OperationKind,
        model: &str,
    ) -> Option<OllamaGuard<'_>> {
        let guard = self.lock.try_lock().ok()?;
        self.set_current_op(kind, model);
        Some(OllamaGuard {
            _guard: guard,
            service: self,
        })
    }

    /// What operation is currently running?
    ///
    /// Returns `None` if Ollama is idle.
    pub fn current_operation(&self) -> Option<ActiveOperation> {
        self.current_op.lock().ok()?.clone()
    }

    /// Is Ollama currently busy with an operation?
    pub fn is_busy(&self) -> bool {
        self.lock.try_lock().is_err()
    }

    // ── Internal ────────────────────────────────────────────

    fn set_current_op(&self, kind: OperationKind, model: &str) {
        if let Ok(mut current) = self.current_op.lock() {
            *current = Some(ActiveOperation {
                kind,
                model: model.to_string(),
                started_at: chrono::Utc::now().to_rfc3339(),
            });
        }
    }

    fn clear_current_op(&self) {
        if let Ok(mut current) = self.current_op.lock() {
            *current = None;
        }
    }
}

impl Default for OllamaService {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════
// OllamaGuard — RAII exclusive access token
// ═══════════════════════════════════════════════════════════

/// RAII guard for exclusive Ollama access.
///
/// Dropping the guard releases the lock and clears the current operation.
/// Hold this guard for the entire duration of your Ollama operation.
pub struct OllamaGuard<'a> {
    _guard: MutexGuard<'a, ()>,
    service: &'a OllamaService,
}

impl Drop for OllamaGuard<'_> {
    fn drop(&mut self) {
        self.service.clear_current_op();
    }
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_service_is_idle() {
        let service = OllamaService::new();
        assert!(!service.is_busy());
        assert!(service.current_operation().is_none());
    }

    #[test]
    fn acquire_sets_current_operation() {
        let service = OllamaService::new();
        let guard = service
            .acquire(OperationKind::DocumentOcr, "deepseek-ocr:latest")
            .unwrap();
        assert!(service.is_busy());

        let op = service.current_operation().unwrap();
        assert_eq!(op.kind, OperationKind::DocumentOcr);
        assert_eq!(op.model, "deepseek-ocr:latest");
        assert!(!op.started_at.is_empty());

        drop(guard);
        assert!(!service.is_busy());
        assert!(service.current_operation().is_none());
    }

    #[test]
    fn try_acquire_returns_none_when_busy() {
        let service = OllamaService::new();
        let _guard = service
            .acquire(OperationKind::ChatGeneration, "medgemma:4b")
            .unwrap();

        // Second acquisition should fail (non-blocking)
        let result = service.try_acquire(OperationKind::ModelVerification, "medgemma:4b");
        assert!(result.is_none());
    }

    #[test]
    fn try_acquire_succeeds_when_idle() {
        let service = OllamaService::new();
        let guard = service.try_acquire(OperationKind::ModelVerification, "medgemma:4b");
        assert!(guard.is_some());
        assert!(service.is_busy());
    }

    #[test]
    fn drop_guard_clears_current_operation() {
        let service = OllamaService::new();

        {
            let _guard = service
                .acquire(OperationKind::BatchExtraction, "medgemma:4b")
                .unwrap();
            assert_eq!(
                service.current_operation().unwrap().kind,
                OperationKind::BatchExtraction,
            );
        }
        // Guard dropped — operation cleared
        assert!(service.current_operation().is_none());
        assert!(!service.is_busy());
    }

    #[test]
    fn acquire_blocks_until_released() {
        use std::sync::Arc;
        use std::thread;

        let service = Arc::new(OllamaService::new());
        let service2 = Arc::clone(&service);

        // Thread 1: acquire and hold for 50ms
        let handle = thread::spawn(move || {
            let _guard = service2
                .acquire(OperationKind::DocumentOcr, "deepseek-ocr:latest")
                .unwrap();
            thread::sleep(std::time::Duration::from_millis(50));
        });

        // Give thread 1 time to acquire
        thread::sleep(std::time::Duration::from_millis(10));

        // Main thread: should block until thread 1 releases
        let start = std::time::Instant::now();
        let _guard = service
            .acquire(OperationKind::ChatGeneration, "medgemma:4b")
            .unwrap();
        let waited = start.elapsed();

        // We should have waited at least ~30ms (50ms - 10ms head start - some margin)
        assert!(
            waited.as_millis() >= 20,
            "Expected to block, but only waited {}ms",
            waited.as_millis()
        );

        handle.join().unwrap();
    }

    #[test]
    fn operation_kind_display() {
        assert_eq!(OperationKind::DocumentOcr.to_string(), "Document OCR");
        assert_eq!(OperationKind::ChatGeneration.to_string(), "Chat generation");
        assert_eq!(OperationKind::BatchExtraction.to_string(), "Batch extraction");
        assert_eq!(OperationKind::ModelVerification.to_string(), "Model verification");
    }

    #[test]
    fn operation_kind_serializes_snake_case() {
        let json = serde_json::to_string(&OperationKind::DocumentOcr).unwrap();
        assert_eq!(json, "\"document_ocr\"");

        let json = serde_json::to_string(&OperationKind::ChatGeneration).unwrap();
        assert_eq!(json, "\"chat_generation\"");
    }

    #[test]
    fn active_operation_serializes() {
        let op = ActiveOperation {
            kind: OperationKind::DocumentOcr,
            model: "deepseek-ocr:latest".to_string(),
            started_at: "2026-02-22T10:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("\"document_ocr\""));
        assert!(json.contains("deepseek-ocr:latest"));
        assert!(json.contains("2026-02-22T10:00:00Z"));
    }

    #[test]
    fn client_factory_returns_valid_client() {
        let client = OllamaService::client();
        assert!(
            client.base_url().contains("localhost") || client.base_url().contains("127.0.0.1"),
        );
    }

    #[test]
    fn default_trait_matches_new() {
        let a = OllamaService::new();
        let b = OllamaService::default();
        assert!(!a.is_busy());
        assert!(!b.is_busy());
    }
}
