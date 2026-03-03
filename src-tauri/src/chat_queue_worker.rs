//! CHAT-QUEUE-01: Chat queue worker — background task processing queued messages.
//!
//! Spawns a tokio task during Tauri setup. Loops: await notification →
//! drain all Queued items → run each through the RAG pipeline →
//! emit `chat-queue-update` events per state change.
//!
//! Pattern: Mirrors ImportQueueWorker (BTL-10 C4) — sequential processing,
//! one item at a time (Ollama serves one request at a time).

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::chat::{ChatStreamEvent, StreamChunkPayload};
use crate::chat_queue::{ChatQueueItem, ChatQueueState};
use crate::core_state::CoreState;
use crate::pipeline::rag::conversation::ConversationManager;
use crate::pipeline::safety::orchestrator::SafetyFilterImpl;

// ---------------------------------------------------------------------------
// Event payload
// ---------------------------------------------------------------------------

/// Event payload emitted to frontend on each queue state change.
#[derive(Debug, Clone, Serialize)]
pub struct ChatQueueEvent {
    pub queue_item_id: String,
    pub conversation_id: String,
    pub patient_message_id: String,
    pub state: ChatQueueState,
    pub queue_position: u32,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Worker entry point
// ---------------------------------------------------------------------------

/// Start the chat queue worker as a background tokio task.
///
/// Call from Tauri `setup`. The task runs for the lifetime of the app.
/// It awaits `ChatQueueService::notifier()` for new items.
pub fn start_chat_queue_worker(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tracing::info!("Chat queue worker started");
        worker_loop(&app_handle).await;
    });
}

/// Main worker loop — await notification, drain all queued items sequentially.
async fn worker_loop(app: &AppHandle) {
    let state: tauri::State<'_, Arc<CoreState>> = app.state();
    let queue = state.chat_queue();

    loop {
        // Wait for enqueue notification
        queue.notifier().notified().await;

        // Drain all queued items (sequential — Ollama serves one request at a time)
        while let Some(item) = queue.next_queued() {
            queue.set_processing(true);
            emit_queue_event(app, &item);
            process_chat_item(app, item).await;
            // FIX-5: Prune oldest terminal items to prevent unbounded memory growth
            queue.prune_terminal(50);
        }

        queue.set_processing(false);
    }
}

// ---------------------------------------------------------------------------
// Event emission
// ---------------------------------------------------------------------------

/// Emit a `chat-queue-update` event from a ChatQueueItem snapshot.
fn emit_queue_event(app: &AppHandle, item: &ChatQueueItem) {
    let _ = app.emit(
        "chat-queue-update",
        ChatQueueEvent {
            queue_item_id: item.id.clone(),
            conversation_id: item.conversation_id.clone(),
            patient_message_id: item.patient_message_id.clone(),
            state: item.state.clone(),
            queue_position: item.queue_position,
            error: item.error.clone(),
        },
    );
}

/// Re-read an item from the queue and emit its current state.
fn emit_current_state(app: &AppHandle, state: &CoreState, item_id: &str) {
    if let Some(item) = state.chat_queue().get_item(item_id) {
        emit_queue_event(app, &item);
    }
}

// ---------------------------------------------------------------------------
// Item processing
// ---------------------------------------------------------------------------

/// Safety timeout for overall chat processing (streaming phase only).
///
/// Why 120s: MedGemma 4B on CPU generates at ~3.2 tok/s (BM-04 benchmark).
/// A 512-token response takes ~160s worst case. 120s covers 95th percentile.
/// Butler acquire is handled separately via polling (ACQUIRE_TIMEOUT).
const PROCESSING_TIMEOUT: Duration = Duration::from_secs(120);

/// Maximum time to wait for Butler lock (FIX-3).
///
/// Why 60s: Document imports take 30-60s. If Butler is still held after 60s,
/// something is likely stuck. Fail fast and let the user retry.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(60);

/// Polling interval for try_acquire (FIX-3).
const ACQUIRE_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Process a single chat queue item with safety timeout.
///
/// Wraps the blocking work in `tokio::time::timeout` to prevent hangs
/// when Ollama crashes or the Butler lock is held indefinitely.
async fn process_chat_item(app: &AppHandle, item: ChatQueueItem) {
    let app_clone = app.clone();
    let item_id_for_error = item.id.clone();

    let result = tokio::time::timeout(
        PROCESSING_TIMEOUT,
        tauri::async_runtime::spawn_blocking(move || {
            process_chat_item_blocking(&app_clone, &item)
        }),
    )
    .await;

    let state: tauri::State<'_, Arc<CoreState>> = app.state();

    match result {
        // Success — state already updated inside blocking function
        Ok(Ok(Ok(()))) => {}
        // Processing error (RAG failure, safety filter, etc.)
        Ok(Ok(Err(err))) => {
            let _ = state.chat_queue().update_state(
                &item_id_for_error,
                ChatQueueState::Failed,
                Some(err.clone()),
            );
            emit_current_state(app, &state, &item_id_for_error);
            tracing::warn!(
                item_id = %item_id_for_error,
                error = %err,
                "Chat queue item failed"
            );
        }
        // spawn_blocking panicked
        Ok(Err(join_err)) => {
            let err = format!("Internal error: {join_err}");
            let _ = state.chat_queue().update_state(
                &item_id_for_error,
                ChatQueueState::Failed,
                Some(err),
            );
            emit_current_state(app, &state, &item_id_for_error);
        }
        // Safety timeout exceeded
        Err(_timeout) => {
            let err =
                "Chat processing timed out (120s). The AI model may be overloaded.".to_string();
            let _ = state.chat_queue().update_state(
                &item_id_for_error,
                ChatQueueState::Failed,
                Some(err),
            );
            emit_current_state(app, &state, &item_id_for_error);
            tracing::warn!(
                item_id = %item_id_for_error,
                "Chat queue item timed out after {}s",
                PROCESSING_TIMEOUT.as_secs()
            );
        }
    }
}

/// Synchronous chat processing — runs on a blocking thread.
///
/// Extracted from `send_chat_message` slow path (commands/chat.rs lines 102-154).
/// Steps:
/// 1. Open DB, parse conversation UUID
/// 2. Resolve model via preferences (L6-04)
/// 3. Acquire Butler lock (blocks until SLM free)
/// 4. Transition to Streaming, emit event
/// 5. Set up token streaming channel + forwarder thread
/// 6. Run RAG pipeline (try_rag_query)
/// 7. Emit filtered/placeholder response + persist AI message
/// 8. Transition to Complete, emit event
fn process_chat_item_blocking(
    app: &AppHandle,
    item: &ChatQueueItem,
) -> Result<(), String> {
    let state: tauri::State<'_, Arc<CoreState>> = app.state();

    // 1. Open DB, parse conversation UUID
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let db_path = state.db_path().map_err(|e| e.to_string())?;
    let conv_uuid = uuid::Uuid::parse_str(&item.conversation_id)
        .map_err(|e| format!("Invalid conversation ID: {e}"))?;

    // I18N-06: Use profile language for safety fallback messages
    let lang = state.get_profile_language();
    let safety = SafetyFilterImpl::with_language(&lang);
    let manager = ConversationManager::new(&conn);

    // 2. Resolve active model via preferences (L6-04)
    let ollama_client = crate::ollama_service::OllamaService::client();
    let resolved_model = state
        .resolver()
        .resolve(&conn, &ollama_client)
        .ok()
        .map(|r| r.name);
    let db_key = state.db_key().ok();

    // 3. BTL-07: Acquire Butler lock via polling (FIX-3: fail-fast on timeout)
    //
    // Why polling instead of blocking acquire(): If the blocking thread hangs
    // forever on acquire(), tokio::time::timeout can mark the item Failed but
    // the thread continues running, holding resources. Polling with try_acquire()
    // means the thread checks periodically and exits cleanly on timeout.
    let model_for_guard = resolved_model.as_deref().unwrap_or("unknown");
    let acquire_start = std::time::Instant::now();
    let _butler_guard = loop {
        if let Some(guard) = state.butler().try_acquire(
            crate::ollama_service::OperationKind::ChatGeneration,
            model_for_guard,
        ) {
            break guard;
        }
        if acquire_start.elapsed() > ACQUIRE_TIMEOUT {
            return Err(format!(
                "Timed out waiting for AI ({}s). Another operation may be in progress — try again.",
                ACQUIRE_TIMEOUT.as_secs()
            ));
        }
        std::thread::sleep(ACQUIRE_POLL_INTERVAL);
    };

    // 4. Transition: Acquiring → Streaming
    let _ = state.chat_queue().update_state(
        &item.id,
        ChatQueueState::Streaming,
        None,
    );
    emit_current_state(app, &state, &item.id);

    // 5. Set up token streaming: tokens flow from RAG → channel → Tauri events
    let (token_tx, token_rx) = std::sync::mpsc::channel::<String>();
    let stream_app = app.clone();
    let stream_conv_id = item.conversation_id.clone();
    let forwarder = std::thread::spawn(move || {
        while let Ok(token) = token_rx.recv() {
            let _ = stream_app.emit(
                "chat-stream",
                &ChatStreamEvent {
                    conversation_id: stream_conv_id.clone(),
                    chunk: StreamChunkPayload::Token { text: token },
                },
            );
        }
    });

    // ME-04: Build demographics from active profile for sex/ethnicity-aware enrichment
    let demographics = state.get_patient_demographics();

    // 6. Run RAG pipeline (ME-03: invariant registry enables clinical enrichment)
    let registry = state.invariants();
    match crate::commands::chat::try_rag_query(
        &item.text,
        conv_uuid,
        &conn,
        &db_path,
        resolved_model.as_deref(),
        db_key.as_ref(),
        registry,
        &lang,
        demographics,
        token_tx,
    ) {
        Some(rag_response) => {
            // Wait for all tokens to be forwarded before emitting Done
            let _ = forwarder.join();
            // 7a. Filter RAG response through safety layers (emits Citations + Done + persists)
            crate::commands::chat::emit_filtered_response(
                app,
                &item.conversation_id,
                &rag_response,
                &safety,
                &manager,
                conv_uuid,
            )?;
            // Implicit verification: successful generation proves model works
            state.set_ai_verified(true);
        }
        None => {
            // Drop sender so forwarder thread exits
            drop(forwarder);
            // S.3: Emit degraded status event when AI pipeline fails
            state.set_ai_verified(false);
            let _ = app.emit("ai-status-changed", crate::commands::StatusLevel::Degraded);
            // 7b. Emit localized placeholder response + persist
            crate::commands::chat::emit_placeholder_response(
                app,
                &item.conversation_id,
                &manager,
                conv_uuid,
                &lang,
            )?;

            // FIX-2: Transition to Failed (not Complete) — user gets error banner
            // with retry option. The placeholder message is already persisted and
            // displayed via Done event above, but the Failed state signals the
            // frontend to show retry UI. (Apple Health pattern: degraded state
            // with clear retry action, not silent success.)
            let _ = state.chat_queue().update_state(
                &item.id,
                ChatQueueState::Failed,
                Some("AI model unavailable — placeholder response provided".into()),
            );
            emit_current_state(app, &state, &item.id);
            state.update_activity();
            tracing::warn!(
                item_id = %item.id,
                conversation_id = %item.conversation_id,
                "Chat queue worker: AI unavailable, placeholder response sent"
            );
            return Ok(());
        }
    }

    // 8. Transition: Streaming → Complete (only reached on successful RAG)
    let _ = state.chat_queue().update_state(
        &item.id,
        ChatQueueState::Complete,
        None,
    );
    emit_current_state(app, &state, &item.id);

    state.update_activity();
    tracing::info!(
        item_id = %item.id,
        conversation_id = %item.conversation_id,
        "Chat queue worker: message processed"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_queue::ChatQueueState;

    #[test]
    fn chat_queue_event_serializes() {
        let event = ChatQueueEvent {
            queue_item_id: "qi-abc-123".into(),
            conversation_id: "conv-456".into(),
            patient_message_id: "msg-789".into(),
            state: ChatQueueState::Queued,
            queue_position: 1,
            error: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("qi-abc-123"));
        assert!(json.contains("conv-456"));
        assert!(json.contains("msg-789"));
        assert!(json.contains("Queued"));
        assert!(json.contains("\"queue_position\":1"));
        assert!(json.contains("\"error\":null"));
    }

    #[test]
    fn chat_queue_event_with_error() {
        let event = ChatQueueEvent {
            queue_item_id: "qi-fail".into(),
            conversation_id: "conv-1".into(),
            patient_message_id: "msg-1".into(),
            state: ChatQueueState::Failed,
            queue_position: 2,
            error: Some("Ollama unreachable".into()),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Failed"));
        assert!(json.contains("Ollama unreachable"));
        assert!(json.contains("\"queue_position\":2"));
    }

    #[test]
    fn chat_queue_event_all_states() {
        for state in [
            ChatQueueState::Queued,
            ChatQueueState::Acquiring,
            ChatQueueState::Streaming,
            ChatQueueState::Complete,
            ChatQueueState::Failed,
        ] {
            let event = ChatQueueEvent {
                queue_item_id: "test".into(),
                conversation_id: "conv".into(),
                patient_message_id: "msg".into(),
                state: state.clone(),
                queue_position: 1,
                error: None,
            };
            let json = serde_json::to_string(&event).unwrap();
            assert!(
                json.contains(&format!("{:?}", state)),
                "State {:?} not found in JSON: {}",
                state,
                json
            );
        }
    }

    #[test]
    fn processing_timeout_is_120s() {
        assert_eq!(PROCESSING_TIMEOUT, Duration::from_secs(120));
    }

    #[test]
    fn acquire_timeout_is_60s() {
        assert_eq!(ACQUIRE_TIMEOUT, Duration::from_secs(60));
    }

    #[test]
    fn acquire_poll_interval_is_500ms() {
        assert_eq!(ACQUIRE_POLL_INTERVAL, Duration::from_millis(500));
    }
}
