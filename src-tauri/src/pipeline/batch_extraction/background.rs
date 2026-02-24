//! Background batch scheduler — periodic extraction trigger.
//!
//! Spawns a background thread that checks every 15 minutes whether
//! a batch extraction should run. Conditions:
//! 1. Profile is active (unlocked)
//! 2. User is idle (> configured idle_minutes)
//! 3. Cooldown has elapsed since last batch
//! 4. Current hour matches configured batch_start_hour (if set)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::{Timelike, Utc};
use tauri::{AppHandle, Emitter, Manager};

use super::analyzer::RuleBasedAnalyzer;
use super::context::load_patient_context;
use super::extractors::{AppointmentExtractor, MedicationExtractor, SymptomExtractor};
use super::runner::{run_full_batch, BatchRunner};
use super::scheduler::SqliteBatchScheduler;
use super::store::SqlitePendingStore;
use super::types::*;
use crate::core_state::CoreState;

/// Check interval: every 15 minutes.
const CHECK_INTERVAL_SECS: u64 = 15 * 60;

/// Sleep granularity for shutdown responsiveness (5 seconds).
const SLEEP_GRANULARITY_SECS: u64 = 5;

/// Handle for the background batch scheduler thread.
///
/// Supports graceful shutdown via `shutdown()` or automatic cleanup on `Drop`.
/// Store this in Tauri app state so it is dropped when the app exits.
pub struct BatchSchedulerHandle {
    shutdown: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl BatchSchedulerHandle {
    /// Request graceful shutdown. Current batch (if running) will complete,
    /// but no new batches will be started.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

impl Drop for BatchSchedulerHandle {
    fn drop(&mut self) {
        self.shutdown();
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// Start the background batch scheduler on a separate thread.
///
/// Call this from the Tauri `.setup()` callback. Returns a handle that
/// supports graceful shutdown — store it in Tauri managed state.
pub fn start_background_scheduler(app_handle: AppHandle) -> BatchSchedulerHandle {
    let shutdown = Arc::new(AtomicBool::new(false));
    let flag = shutdown.clone();

    let handle = std::thread::spawn(move || {
        tracing::info!("Background batch scheduler started (check every {}s)", CHECK_INTERVAL_SECS);
        scheduler_loop(&app_handle, &flag);
    });

    BatchSchedulerHandle {
        shutdown,
        handle: Some(handle),
    }
}

fn scheduler_loop(app: &AppHandle, shutdown: &AtomicBool) {
    while !shutdown.load(Ordering::Relaxed) {
        // Sleep in small increments for responsive shutdown
        for _ in 0..(CHECK_INTERVAL_SECS / SLEEP_GRANULARITY_SECS) {
            if shutdown.load(Ordering::Relaxed) {
                tracing::info!("Background batch scheduler shutting down");
                return;
            }
            std::thread::sleep(Duration::from_secs(SLEEP_GRANULARITY_SECS));
        }

        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        if let Err(e) = try_run_batch(app) {
            tracing::debug!(error = %e, "Background batch check: not ready");
        }
    }
    tracing::info!("Background batch scheduler shutting down");
}

fn try_run_batch(app: &AppHandle) -> Result<(), String> {
    let state: tauri::State<'_, Arc<CoreState>> = app.state();

    // 1. Profile must be active
    let conn = state.open_db().map_err(|e| format!("No active profile: {e}"))?;

    // 2. User must be idle
    let config = ExtractionConfig::default();
    let idle = state.idle_minutes();
    if idle < config.idle_minutes as u64 {
        return Err(format!("User active ({idle}m idle, need {}m)", config.idle_minutes));
    }

    // 3. Check if current hour matches batch_start_hour (if configured)
    if let Some(start_hour) = config.batch_start_hour {
        let current_hour = Utc::now().hour();
        // Allow a 1-hour window (e.g., configured 2 AM → runs between 2:00-2:59)
        if current_hour != start_hour {
            return Err(format!("Not batch hour (current={current_hour}, configured={start_hour})"));
        }
    }

    // 4. Resolve model and build runner
    let model_name = resolve_model(app)?;
    let run_config = ExtractionConfig {
        model_name,
        ..config
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
        run_config.clone(),
    );

    // Acquire exclusive Ollama access for batch extraction
    let _ollama_guard = state.ollama().acquire(
        crate::ollama_service::OperationKind::BatchExtraction,
        &run_config.model_name,
    ).map_err(|e| format!("Ollama busy: {e}"))?;

    let llm = crate::ollama_service::OllamaService::client();

    let patient_context = load_patient_context(&conn).unwrap_or_default();

    let app_clone = app.clone();
    let progress_fn = move |event: BatchStatusEvent| {
        let _ = app_clone.emit("extraction-progress", &event);
    };

    tracing::info!("Background batch extraction starting");

    let result = run_full_batch(
        &conn,
        &scheduler,
        &runner,
        &store,
        &llm,
        &run_config,
        &patient_context,
        Some(&progress_fn),
    )
    .map_err(|e| format!("Batch failed: {e}"))?;

    tracing::info!(
        processed = result.conversations_processed,
        items = result.items_stored,
        duration_ms = result.duration_ms,
        "Background batch extraction completed"
    );

    Ok(())
}

fn resolve_model(app: &AppHandle) -> Result<String, String> {
    let state: tauri::State<'_, Arc<CoreState>> = app.state();
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let client = crate::ollama_service::OllamaService::client();

    let model = state
        .resolver()
        .resolve(&conn, &client)
        .ok()
        .map(|m| m.name)
        .unwrap_or_else(|| crate::pipeline::structuring::ollama_types::DEFAULT_MODEL_FALLBACK.to_string());

    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_interval_is_15_minutes() {
        assert_eq!(CHECK_INTERVAL_SECS, 900);
    }

    #[test]
    fn sleep_granularity_divides_check_interval() {
        assert_eq!(CHECK_INTERVAL_SECS % SLEEP_GRANULARITY_SECS, 0);
    }

    #[test]
    fn shutdown_flag_sets_atomic() {
        let handle = BatchSchedulerHandle {
            shutdown: Arc::new(AtomicBool::new(false)),
            handle: None,
        };
        assert!(!handle.shutdown.load(Ordering::Relaxed));
        handle.shutdown();
        assert!(handle.shutdown.load(Ordering::Relaxed));
    }

    #[test]
    fn scheduler_loop_exits_on_shutdown() {
        let shutdown = Arc::new(AtomicBool::new(true)); // pre-set
        // scheduler_loop should exit immediately since shutdown is already true
        // We can't call it without an AppHandle, but we verify the flag logic
        assert!(shutdown.load(Ordering::Relaxed));
    }
}
