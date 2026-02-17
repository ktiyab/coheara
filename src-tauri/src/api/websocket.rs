//! M0-03: WebSocket layer for real-time desktop → phone communication.
//!
//! Handles WebSocket upgrade, heartbeat (30s), session max (1h), message routing,
//! and per-connection rate limiting (10 incoming messages/sec).
//!
//! Connection lifecycle:
//! 1. Phone calls `POST /api/auth/ws-ticket` to get a one-time ticket
//! 2. Phone opens `GET /ws/connect?ticket=xxx` — ticket validated, WS upgraded
//! 3. Server sends Welcome, flushes pending alerts
//! 4. Heartbeat every 30s — 3 missed = disconnect
//! 5. Session max 1h — warning at 59 min, close at 60 min

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::api::error::ApiError;
use crate::api::types::ApiContext;
use crate::core_state::CoreState;
use crate::device_manager::{WsIncoming, WsOutgoing};

/// Heartbeat interval: server sends Heartbeat every 30 seconds.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

/// Maximum WebSocket session duration: 1 hour (Nadia requirement).
const SESSION_MAX: Duration = Duration::from_secs(3600);

/// Warning sent this many seconds before session expiry.
const EXPIRY_WARNING: Duration = Duration::from_secs(60);

/// Disconnect after this many missed heartbeats (3 × 30s = 90s).
const MAX_MISSED_HEARTBEATS: u32 = 3;

/// Maximum incoming messages per second per connection.
const MAX_INCOMING_PER_SECOND: u32 = 10;

/// Query parameters for WebSocket upgrade.
#[derive(Deserialize)]
pub struct WsAuthQuery {
    ticket: String,
}

// ═══════════════════════════════════════════════════════════
// WsSessionState — Testable session state (RS-M0-03-002)
// ═══════════════════════════════════════════════════════════

/// Action returned by `WsSessionState::on_heartbeat_tick()`.
#[derive(Debug, PartialEq)]
pub(crate) enum HeartbeatAction {
    /// Send a heartbeat and continue.
    SendHeartbeat,
    /// Send an expiry warning with seconds remaining.
    SendExpiryWarning { seconds_remaining: u32 },
    /// Session has expired — disconnect.
    SessionExpired,
    /// Too many missed heartbeats — disconnect.
    HeartbeatTimeout,
}

/// Testable WebSocket session state.
///
/// Extracted from `handle_ws` to enable unit testing of heartbeat,
/// session max, and rate limiting logic without a live WebSocket.
pub(crate) struct WsSessionState {
    session_start: Instant,
    missed_heartbeats: u32,
    expiry_warned: bool,
    incoming_times: VecDeque<Instant>,
}

impl WsSessionState {
    fn new() -> Self {
        Self {
            session_start: Instant::now(),
            missed_heartbeats: 0,
            expiry_warned: false,
            incoming_times: VecDeque::new(),
        }
    }

    #[cfg(test)]
    fn with_start(start: Instant) -> Self {
        Self {
            session_start: start,
            missed_heartbeats: 0,
            expiry_warned: false,
            incoming_times: VecDeque::new(),
        }
    }

    /// Called when a Pong is received from the phone.
    fn on_pong(&mut self) {
        self.missed_heartbeats = 0;
    }

    /// Called on each heartbeat tick. Returns the action to take.
    fn on_heartbeat_tick(&mut self) -> HeartbeatAction {
        let elapsed = self.session_start.elapsed();

        // Session max (1 hour)
        if elapsed >= SESSION_MAX {
            return HeartbeatAction::SessionExpired;
        }

        // Expiry warning (60s before)
        if !self.expiry_warned && elapsed >= SESSION_MAX - EXPIRY_WARNING {
            let remaining = (SESSION_MAX - elapsed).as_secs() as u32;
            self.expiry_warned = true;
            return HeartbeatAction::SendExpiryWarning {
                seconds_remaining: remaining,
            };
        }

        // Missed heartbeats → disconnect
        if self.missed_heartbeats >= MAX_MISSED_HEARTBEATS {
            return HeartbeatAction::HeartbeatTimeout;
        }

        // Normal: send heartbeat and increment miss counter
        self.missed_heartbeats += 1;
        HeartbeatAction::SendHeartbeat
    }

    /// Check incoming rate limit. Returns true if allowed.
    fn check_rate(&mut self) -> bool {
        check_incoming_rate(&mut self.incoming_times)
    }
}

/// WebSocket upgrade handler.
///
/// Validates the one-time ticket before upgrading the connection.
/// The ticket was obtained via `POST /api/auth/ws-ticket`.
pub async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(ctx): State<ApiContext>,
    Query(query): Query<WsAuthQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let (device_id, device_name) = {
        let mut tickets = ctx
            .ws_tickets
            .lock()
            .map_err(|_| ApiError::Internal("ticket lock".into()))?;
        tickets
            .consume(&query.ticket)
            .ok_or(ApiError::Unauthorized)?
    };

    tracing::info!(device_id = %device_id, "WebSocket upgrade accepted");
    let core = ctx.core.clone();
    Ok(ws.on_upgrade(move |socket| handle_ws(socket, core, device_id, device_name)))
}

/// Main WebSocket connection handler.
///
/// Spawns a sender task for channel→WS forwarding, then runs the
/// receive + heartbeat loop until disconnect or session expiry.
async fn handle_ws(
    socket: WebSocket,
    core: Arc<CoreState>,
    device_id: String,
    _device_name: String,
) {
    let (ws_sink, mut ws_stream) = socket.split();
    let (tx, rx) = mpsc::channel::<WsOutgoing>(64);

    // Register WS channel in DeviceManager
    {
        let mut devices = match core.write_devices() {
            Ok(d) => d,
            Err(_) => return,
        };
        devices.register_ws(&device_id, tx.clone());
    }

    // Spawn sender task (reads from channel, writes to WebSocket)
    let sender_handle = tokio::spawn(async move {
        let mut sink = ws_sink;
        let mut rx = rx;
        while let Some(msg) = rx.recv().await {
            let json = match serde_json::to_string(&msg) {
                Ok(j) => j,
                Err(_) => continue,
            };
            if sink.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    // Send Welcome message with reconnection policy
    let profile_name = core
        .read_session()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.profile_name.clone()))
        .unwrap_or_else(|| "Unknown".to_string());
    let session_id = uuid::Uuid::new_v4().to_string();
    let _ = tx
        .send(WsOutgoing::Welcome {
            profile_name,
            session_id,
            reconnect_policy: crate::device_manager::ReconnectionPolicy::default(),
        })
        .await;

    // Flush pending alerts (queued while device was disconnected)
    if let Ok(mut devices) = core.write_devices() {
        devices.flush_pending(&device_id);
    }

    // Main receive + heartbeat loop
    let mut session = WsSessionState::new();
    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
    heartbeat.tick().await; // Consume initial immediate tick

    loop {
        tokio::select! {
            msg = ws_stream.next() => {
                match msg {
                    Some(Ok(Message::Text(ref text))) => {
                        if !session.check_rate() {
                            continue;
                        }
                        if let Ok(incoming) = serde_json::from_str::<WsIncoming>(text) {
                            match incoming {
                                WsIncoming::Pong {} => {
                                    session.on_pong();
                                }
                                WsIncoming::Ready {} => {}
                                other => {
                                    handle_incoming(&core, &device_id, other, &tx).await;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {} // Ping/Pong handled by axum/tungstenite
                }
            }
            _ = heartbeat.tick() => {
                match session.on_heartbeat_tick() {
                    HeartbeatAction::SessionExpired => {
                        let _ = tx.send(WsOutgoing::SessionExpiring {
                            seconds_remaining: 0,
                        }).await;
                        break;
                    }
                    HeartbeatAction::SendExpiryWarning { seconds_remaining } => {
                        let _ = tx.send(WsOutgoing::SessionExpiring {
                            seconds_remaining,
                        }).await;
                        // Don't break — continue session until expired
                    }
                    HeartbeatAction::HeartbeatTimeout => {
                        tracing::info!(
                            device_id = %device_id,
                            "{MAX_MISSED_HEARTBEATS} missed heartbeats, disconnecting"
                        );
                        break;
                    }
                    HeartbeatAction::SendHeartbeat => {
                        let _ = tx.send(WsOutgoing::Heartbeat {
                            server_time: chrono::Utc::now().to_rfc3339(),
                        }).await;
                    }
                }
            }
        }
    }

    // Cleanup: drop sender (stops sender task), unregister WS
    drop(tx);
    let _ = sender_handle.await;

    if let Ok(mut devices) = core.write_devices() {
        devices.unregister_ws(&device_id);
    }

    tracing::info!(device_id = %device_id, "WebSocket disconnected");
}

/// Check if an incoming message is within the rate limit (10/sec).
///
/// Returns `true` if allowed, `false` if rate-limited.
fn check_incoming_rate(timestamps: &mut VecDeque<Instant>) -> bool {
    let now = Instant::now();
    let one_sec_ago = now - Duration::from_secs(1);

    // Remove timestamps older than 1 second
    while let Some(&front) = timestamps.front() {
        if front < one_sec_ago {
            timestamps.pop_front();
        } else {
            break;
        }
    }

    if timestamps.len() as u32 >= MAX_INCOMING_PER_SECOND {
        return false;
    }

    timestamps.push_back(now);
    true
}

/// Handle incoming messages from the phone (except Pong/Ready).
async fn handle_incoming(
    core: &Arc<CoreState>,
    device_id: &str,
    msg: WsIncoming,
    tx: &mpsc::Sender<WsOutgoing>,
) {
    match msg {
        WsIncoming::ChatQuery {
            conversation_id,
            message,
        } => {
            handle_chat_query(core, device_id, conversation_id, message, tx).await;
        }
        WsIncoming::ChatFeedback {
            conversation_id: _,
            message_id,
            helpful,
        } => {
            handle_chat_feedback(core, device_id, &message_id, helpful);
        }
        _ => {} // Ready and Pong handled in main loop
    }
}

/// Handle a chat query from the phone: sanitize → save → RAG → safety filter → stream back.
async fn handle_chat_query(
    core: &Arc<CoreState>,
    device_id: &str,
    conversation_id: Option<String>,
    message: String,
    tx: &mpsc::Sender<WsOutgoing>,
) {
    use crate::chat;
    use crate::device_manager::CitationRef;
    use crate::pipeline::rag::conversation::ConversationManager;
    use crate::pipeline::safety::orchestrator::SafetyFilterImpl;
    use crate::pipeline::safety::types::{FilterOutcome, SafetyFilter};

    let core = core.clone();
    let tx_blocking = tx.clone();
    let tx_error = tx.clone();
    let device_id = device_id.to_string();
    let conv_id_for_error = conversation_id.clone();

    // Run blocking DB + RAG work on a dedicated thread
    let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
        let tx = tx_blocking;
        let db_path = core.db_path().map_err(|e| e.to_string())?;
        let conn = core.open_db().map_err(|e| e.to_string())?;
        let safety = SafetyFilterImpl::new();
        let manager = ConversationManager::new(&conn);

        // 1. Sanitize patient input (L2-02)
        let sanitized = safety
            .sanitize_input(&message)
            .map_err(|e| format!("Input sanitization failed: {e}"))?;

        if sanitized.was_modified {
            tracing::info!(
                modifications = sanitized.modifications.len(),
                "WS patient input sanitized"
            );
        }

        // 2. Create or reuse conversation
        let conv_uuid = match &conversation_id {
            Some(id) => uuid::Uuid::parse_str(id)
                .map_err(|e| format!("Invalid conversation ID: {e}"))?,
            None => manager
                .start(Some("New conversation"))
                .map_err(|e| e.to_string())?,
        };
        let conv_id_str = conv_uuid.to_string();

        // 3. Save patient message
        manager
            .add_patient_message(conv_uuid, &sanitized.text)
            .map_err(|e| e.to_string())?;

        // 4. Update title from first message
        let history = manager.get_history(conv_uuid).map_err(|e| e.to_string())?;
        let patient_count = history
            .iter()
            .filter(|m| m.role == crate::models::enums::MessageRole::Patient)
            .count();
        if patient_count == 1 {
            let title = chat::generate_title(&sanitized.text);
            let _ = chat::update_conversation_title(&conn, &conv_id_str, &title);
        }

        // 5. Resolve active model via preferences (L6-04), then try RAG pipeline
        let ollama_client = crate::pipeline::structuring::ollama::OllamaClient::default_local();
        let resolved_model = core
            .resolver()
            .resolve(&conn, &ollama_client)
            .ok()
            .map(|r| r.name);
        let db_key = core.db_key().ok();
        let rag_response = try_ws_rag_query(&sanitized.text, conv_uuid, &conn, &db_path, resolved_model.as_deref(), db_key.as_ref());

        match rag_response {
            Some(response) => {
                // 6. Safety filter on RAG output
                let filtered = safety
                    .filter_response(&response)
                    .map_err(|e| format!("Safety filter error: {e}"))?;

                let (display_text, _confidence, _boundary) = match &filtered.filter_outcome {
                    FilterOutcome::Passed => (
                        filtered.text.clone(),
                        filtered.confidence,
                        format!("{:?}", filtered.boundary_check),
                    ),
                    FilterOutcome::Rephrased { .. } => {
                        tracing::info!("WS safety filter rephrased RAG response");
                        (
                            filtered.text.clone(),
                            filtered.confidence,
                            format!("{:?}", filtered.boundary_check),
                        )
                    }
                    FilterOutcome::Blocked { fallback_message, .. } => {
                        tracing::warn!("WS safety filter blocked RAG response");
                        (fallback_message.clone(), 0.0, "OutOfBounds".to_string())
                    }
                };

                // 7. Send response token via WS (single token for sync pipeline)
                let _ = tx.blocking_send(WsOutgoing::ChatToken {
                    conversation_id: conv_id_str.clone(),
                    token: display_text.clone(),
                });

                // 8. Build citation refs and send ChatComplete
                let citations: Vec<CitationRef> = response
                    .citations
                    .iter()
                    .map(|c| CitationRef {
                        document_id: c.document_id.to_string(),
                        document_title: c.document_title.clone(),
                        chunk_id: None,
                    })
                    .collect();

                let _ = tx.blocking_send(WsOutgoing::ChatComplete {
                    conversation_id: conv_id_str.clone(),
                    citations,
                });

                // 9. Persist filtered response
                let source_json = if response.citations.is_empty() {
                    None
                } else {
                    serde_json::to_string(
                        &response
                            .citations
                            .iter()
                            .map(|c| c.document_id.to_string())
                            .collect::<Vec<_>>(),
                    )
                    .ok()
                };
                manager
                    .add_response(conv_uuid, &display_text, source_json.as_deref(), filtered.confidence)
                    .map_err(|e| e.to_string())?;
            }
            None => {
                // No AI available — send placeholder
                let placeholder = "I'm not connected to the AI assistant yet. \
                    Please make sure Ollama is running with the MedGemma model on the desktop.";
                let _ = tx.blocking_send(WsOutgoing::ChatToken {
                    conversation_id: conv_id_str.clone(),
                    token: placeholder.to_string(),
                });
                let _ = tx.blocking_send(WsOutgoing::ChatComplete {
                    conversation_id: conv_id_str.clone(),
                    citations: vec![],
                });
                manager
                    .add_response(conv_uuid, placeholder, None, 0.0)
                    .map_err(|e| e.to_string())?;
            }
        }

        core.log_access(
            crate::core_state::AccessSource::MobileDevice {
                device_id: device_id.clone(),
            },
            "chat_query",
            &format!("conversation:{conv_id_str}"),
        );
        core.update_activity();

        Ok(())
    })
    .await;

    match result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            tracing::error!(error = %e, "WS chat query failed");
            let conv_id = conv_id_for_error
                .unwrap_or_else(|| "unknown".to_string());
            let _ = tx_error
                .send(WsOutgoing::ChatError {
                    conversation_id: conv_id,
                    error: "An error occurred processing your question. Please try again.".into(),
                })
                .await;
        }
        Err(e) => {
            tracing::error!(error = %e, "WS chat query task panicked");
        }
    }
}

/// Attempt to run the RAG pipeline for a WebSocket query.
///
/// Uses production components (same as chat commands):
/// - **LLM**: OllamaRagGenerator (MedGemma via local Ollama)
/// - **Embedder**: OnnxEmbedder when available; MockEmbedder fallback
/// - **Vector store**: SqliteVectorStore (persistent, brute-force cosine similarity)
fn try_ws_rag_query(
    query_text: &str,
    conversation_id: uuid::Uuid,
    conn: &rusqlite::Connection,
    db_path: &std::path::Path,
    resolved_model: Option<&str>,
    db_key: Option<&[u8; 32]>,
) -> Option<crate::pipeline::rag::types::RagResponse> {
    use crate::pipeline::rag::ollama::OllamaRagGenerator;
    use crate::pipeline::rag::orchestrator::DocumentRagPipeline;
    use crate::pipeline::rag::types::PatientQuery;
    use crate::pipeline::storage::vectordb::SqliteVectorStore;

    // Use preference-resolved model if available (L6-04)
    let model_name = resolved_model?;
    let generator = OllamaRagGenerator::with_resolved_model(model_name.to_string())?;
    let vector_store = SqliteVectorStore::new(db_path.to_path_buf(), db_key.copied());
    let embedder = crate::pipeline::storage::embedder::build_embedder();

    let pipeline = DocumentRagPipeline::new(&generator, &embedder, &vector_store, conn);
    let query = PatientQuery {
        text: query_text.to_string(),
        conversation_id,
        query_type: None,
    };

    match pipeline.generate(&query) {
        Ok(response) => {
            tracing::info!(
                confidence = response.confidence,
                boundary = ?response.boundary_check,
                citations = response.citations.len(),
                "WS RAG pipeline generated response"
            );
            Some(response)
        }
        Err(e) => {
            tracing::warn!(error = %e, "WS RAG pipeline failed, falling back to placeholder");
            None
        }
    }
}

/// Handle chat feedback from the phone: persist in DB + audit log.
fn handle_chat_feedback(
    core: &Arc<CoreState>,
    device_id: &str,
    message_id: &str,
    helpful: bool,
) {
    use crate::models::enums::MessageFeedback;
    use crate::pipeline::rag::conversation::ConversationManager;

    // Persist feedback
    if let Ok(conn) = core.open_db() {
        if let Ok(msg_uuid) = uuid::Uuid::parse_str(message_id) {
            let manager = ConversationManager::new(&conn);
            let feedback = if helpful {
                MessageFeedback::Helpful
            } else {
                MessageFeedback::NotHelpful
            };
            if let Err(e) = manager.set_feedback(msg_uuid, feedback) {
                tracing::warn!(error = %e, message_id = %message_id, "Failed to persist chat feedback");
            }
        }
    }

    // Audit log
    core.log_access(
        crate::core_state::AccessSource::MobileDevice {
            device_id: device_id.to_string(),
        },
        if helpful {
            "chat_feedback_positive"
        } else {
            "chat_feedback_negative"
        },
        &format!("message:{message_id}"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incoming_rate_allows_under_limit() {
        let mut timestamps = VecDeque::new();
        for _ in 0..9 {
            assert!(check_incoming_rate(&mut timestamps));
        }
    }

    #[test]
    fn incoming_rate_blocks_over_limit() {
        let mut timestamps = VecDeque::new();
        for _ in 0..10 {
            check_incoming_rate(&mut timestamps);
        }
        assert!(!check_incoming_rate(&mut timestamps));
    }

    #[test]
    fn incoming_rate_resets_after_window() {
        let mut timestamps = VecDeque::new();
        let old = Instant::now() - Duration::from_secs(2);

        // Fill with 10 old timestamps
        for _ in 0..10 {
            timestamps.push_back(old);
        }

        // Should be allowed since old timestamps are > 1 second ago
        assert!(check_incoming_rate(&mut timestamps));
    }

    // === WsSessionState tests ===

    #[test]
    fn session_state_sends_heartbeat_on_first_tick() {
        let state_start = Instant::now();
        let mut session = WsSessionState::with_start(state_start);
        assert_eq!(session.on_heartbeat_tick(), HeartbeatAction::SendHeartbeat);
    }

    #[test]
    fn session_state_increments_missed_heartbeats() {
        let mut session = WsSessionState::with_start(Instant::now());
        // Each tick increments the missed counter before returning SendHeartbeat
        session.on_heartbeat_tick(); // missed = 1
        session.on_heartbeat_tick(); // missed = 2
        assert_eq!(session.on_heartbeat_tick(), HeartbeatAction::SendHeartbeat); // missed = 3
        // 4th tick: missed_heartbeats == MAX (3) → timeout
        assert_eq!(session.on_heartbeat_tick(), HeartbeatAction::HeartbeatTimeout);
    }

    #[test]
    fn session_state_pong_resets_missed_counter() {
        let mut session = WsSessionState::with_start(Instant::now());
        session.on_heartbeat_tick(); // missed = 1
        session.on_heartbeat_tick(); // missed = 2
        session.on_pong();           // missed = 0
        // Now 3 more ticks needed before timeout
        assert_eq!(session.on_heartbeat_tick(), HeartbeatAction::SendHeartbeat); // missed = 1
        assert_eq!(session.on_heartbeat_tick(), HeartbeatAction::SendHeartbeat); // missed = 2
        assert_eq!(session.on_heartbeat_tick(), HeartbeatAction::SendHeartbeat); // missed = 3
        assert_eq!(session.on_heartbeat_tick(), HeartbeatAction::HeartbeatTimeout);
    }

    #[test]
    fn session_state_expiry_warning_at_59_minutes() {
        // Start 59 minutes ago → triggers expiry warning
        let start = Instant::now() - Duration::from_secs(59 * 60);
        let mut session = WsSessionState::with_start(start);
        let action = session.on_heartbeat_tick();
        match action {
            HeartbeatAction::SendExpiryWarning { seconds_remaining } => {
                assert!(seconds_remaining <= 60, "should have ≤60s remaining");
            }
            other => panic!("expected SendExpiryWarning, got {other:?}"),
        }
    }

    #[test]
    fn session_state_expiry_warning_sent_only_once() {
        let start = Instant::now() - Duration::from_secs(59 * 60);
        let mut session = WsSessionState::with_start(start);
        // First tick → expiry warning
        assert!(matches!(
            session.on_heartbeat_tick(),
            HeartbeatAction::SendExpiryWarning { .. }
        ));
        // Second tick → normal heartbeat (warning already sent, session not yet expired)
        assert_eq!(session.on_heartbeat_tick(), HeartbeatAction::SendHeartbeat);
    }

    #[test]
    fn session_state_expired_after_one_hour() {
        let start = Instant::now() - Duration::from_secs(3601);
        let mut session = WsSessionState::with_start(start);
        assert_eq!(session.on_heartbeat_tick(), HeartbeatAction::SessionExpired);
    }

    #[test]
    fn session_state_expired_takes_priority_over_heartbeat_timeout() {
        let start = Instant::now() - Duration::from_secs(3601);
        let mut session = WsSessionState::with_start(start);
        // Even with MAX missed heartbeats, expiry takes priority
        session.missed_heartbeats = MAX_MISSED_HEARTBEATS;
        assert_eq!(session.on_heartbeat_tick(), HeartbeatAction::SessionExpired);
    }

    #[test]
    fn session_state_check_rate_delegates_to_rate_limiter() {
        let mut session = WsSessionState::new();
        // First 10 should pass
        for _ in 0..10 {
            assert!(session.check_rate());
        }
        // 11th should be rate-limited
        assert!(!session.check_rate());
    }

    // ═══════════════════════════════════════════════════════════
    // Integration tests — full WebSocket connection lifecycle
    // ═══════════════════════════════════════════════════════════

    use crate::api::router::mobile_api_router_with_ctx;
    use crate::api::types::{generate_token, hash_token, ApiContext};
    use crate::core_state::CoreState;
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite;

    /// Start a test server with shared `ApiContext`, register a device,
    /// issue a WS ticket, and return the URL + core + server handle.
    async fn setup_ws_server() -> (String, Arc<CoreState>, tokio::task::JoinHandle<()>) {
        let core = Arc::new(CoreState::new());
        let token = generate_token();
        let hash = hash_token(&token);

        // Register a paired device
        {
            let mut devices = core.write_devices().unwrap();
            devices
                .register_device(
                    "ws-test-device".to_string(),
                    "Test Phone".to_string(),
                    "TestModel".to_string(),
                    hash,
                )
                .unwrap();
        }

        // Shared context — same instance used by router and test
        let ctx = ApiContext::new(core.clone());

        // Issue a WS ticket directly
        let ticket = {
            let mut tickets = ctx.ws_tickets.lock().unwrap();
            tickets.issue("ws-test-device".to_string(), "Test Phone".to_string())
        };

        let app = mobile_api_router_with_ctx(ctx);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let url = format!("ws://127.0.0.1:{}/ws/connect?ticket={}", addr.port(), ticket);
        (url, core, handle)
    }

    #[tokio::test]
    async fn ws_connect_receives_welcome_message() {
        let (url, _core, server) = setup_ws_server().await;

        let (mut ws, _) = tokio_tungstenite::connect_async(&url)
            .await
            .expect("WS connect failed");

        // First message should be Welcome
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("timeout waiting for Welcome")
            .expect("stream ended")
            .expect("WS error");

        let text = msg.into_text().expect("not text");
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["type"], "Welcome");
        assert!(parsed["session_id"].is_string());
        assert!(parsed["profile_name"].is_string());

        // IMP-020: Verify reconnection policy is present
        let policy = &parsed["reconnect_policy"];
        assert!(policy.is_object(), "Welcome must include reconnect_policy");
        assert_eq!(policy["initial_delay_ms"], 1000);
        assert_eq!(policy["max_delay_ms"], 30000);
        assert_eq!(policy["max_retries"], 10);
        assert_eq!(policy["jitter_ms"], 500);

        let _ = ws.close(None).await;
        server.abort();
    }

    #[tokio::test]
    async fn ws_receives_heartbeat_within_interval() {
        let (url, _core, server) = setup_ws_server().await;

        let (mut ws, _) = tokio_tungstenite::connect_async(&url)
            .await
            .expect("WS connect failed");

        // Skip Welcome
        let _ = ws.next().await;

        // Wait for first Heartbeat (30s interval, allow extra margin)
        let msg = tokio::time::timeout(Duration::from_secs(35), ws.next())
            .await
            .expect("timeout waiting for Heartbeat")
            .expect("stream ended")
            .expect("WS error");

        let text = msg.into_text().expect("not text");
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["type"], "Heartbeat");
        assert!(parsed["server_time"].is_string());

        let _ = ws.close(None).await;
        server.abort();
    }

    #[tokio::test]
    async fn ws_pong_keeps_connection_alive() {
        let (url, _core, server) = setup_ws_server().await;

        let (mut ws, _) = tokio_tungstenite::connect_async(&url)
            .await
            .expect("WS connect failed");

        // Skip Welcome
        let _ = ws.next().await;

        // Wait for Heartbeat, respond with Pong
        let _ = tokio::time::timeout(Duration::from_secs(35), ws.next()).await;

        let pong = serde_json::json!({"type": "Pong"}).to_string();
        ws.send(tungstenite::Message::Text(pong))
            .await
            .expect("send Pong failed");

        // Wait for next Heartbeat — connection should still be alive
        let msg = tokio::time::timeout(Duration::from_secs(35), ws.next())
            .await
            .expect("timeout waiting for 2nd Heartbeat")
            .expect("stream ended")
            .expect("WS error");

        let text = msg.into_text().expect("not text");
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["type"], "Heartbeat");

        let _ = ws.close(None).await;
        server.abort();
    }

    #[tokio::test]
    async fn ws_invalid_ticket_rejects_upgrade() {
        let core = Arc::new(CoreState::new());
        let ctx = ApiContext::new(core);
        let app = mobile_api_router_with_ctx(ctx);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let url = format!("ws://127.0.0.1:{}/ws/connect?ticket=invalid", addr.port());
        let result = tokio_tungstenite::connect_async(&url).await;

        // Should fail — invalid ticket returns HTTP 401 before WS upgrade
        assert!(result.is_err(), "Should reject invalid ticket");

        server.abort();
    }

    #[tokio::test]
    async fn ws_connect_registers_ws_channel() {
        let core = Arc::new(CoreState::new());
        let token = generate_token();
        let hash = hash_token(&token);

        {
            let mut devices = core.write_devices().unwrap();
            devices
                .register_device(
                    "reg-test-device".to_string(),
                    "Test Phone".to_string(),
                    "Model".to_string(),
                    hash,
                )
                .unwrap();
        }

        let ctx = ApiContext::new(core.clone());
        let ticket = {
            let mut tickets = ctx.ws_tickets.lock().unwrap();
            tickets.issue("reg-test-device".to_string(), "Test Phone".to_string())
        };

        let app = mobile_api_router_with_ctx(ctx);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let url = format!("ws://127.0.0.1:{}/ws/connect?ticket={}", addr.port(), ticket);
        let (mut ws, _) = tokio_tungstenite::connect_async(&url)
            .await
            .expect("WS connect failed");

        // Wait for Welcome to confirm server processed the connection
        let _ = ws.next().await;

        // Give server a moment to register the WS channel
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Verify WS channel is registered in DeviceManager
        {
            let devices = core.read_devices().unwrap();
            assert!(
                devices.ws_sender("reg-test-device").is_some(),
                "WS channel should be registered after connect"
            );
        }

        let _ = ws.close(None).await;
        server.abort();
    }

    #[tokio::test]
    async fn ws_ticket_consumed_after_use() {
        let core = Arc::new(CoreState::new());
        let token = generate_token();
        let hash = hash_token(&token);

        {
            let mut devices = core.write_devices().unwrap();
            devices
                .register_device(
                    "reuse-test".to_string(),
                    "Phone".to_string(),
                    "Model".to_string(),
                    hash,
                )
                .unwrap();
        }

        let ctx = ApiContext::new(core.clone());
        let ticket = {
            let mut tickets = ctx.ws_tickets.lock().unwrap();
            tickets.issue("reuse-test".to_string(), "Phone".to_string())
        };

        let app = mobile_api_router_with_ctx(ctx);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let url = format!("ws://127.0.0.1:{}/ws/connect?ticket={}", addr.port(), ticket);

        // First connection succeeds
        let (mut ws, _) = tokio_tungstenite::connect_async(&url)
            .await
            .expect("first connect should succeed");
        let _ = ws.close(None).await;

        // Second connection with same ticket should fail (one-time use)
        tokio::time::sleep(Duration::from_millis(100)).await;
        let result = tokio_tungstenite::connect_async(&url).await;
        assert!(result.is_err(), "Reused ticket should be rejected");

        server.abort();
    }

    // NOTE: Cleanup test (unregister_ws after disconnect) is covered at the unit
    // level by device_manager::tests::unregister_ws_clears_channel. Integration
    // testing requires waiting for 3 missed heartbeats (~90s on WSL2) because
    // SplitStream doesn't detect TCP close — impractical for CI.

    #[tokio::test]
    async fn ws_malformed_json_keeps_connection() {
        let (url, _core, server) = setup_ws_server().await;
        let (mut ws, _) = tokio_tungstenite::connect_async(&url)
            .await
            .expect("WS connect failed");

        // Skip Welcome
        let _ = ws.next().await;

        // Send malformed JSON — should be silently ignored
        ws.send(tungstenite::Message::Text("not valid json {{{".into()))
            .await
            .expect("send malformed failed");
        ws.send(tungstenite::Message::Text("{\"type\": \"Unknown\"}".into()))
            .await
            .expect("send unknown type failed");

        // Connection should still be alive — wait for next Heartbeat
        let msg = tokio::time::timeout(Duration::from_secs(35), ws.next())
            .await
            .expect("timeout — connection should still be alive")
            .expect("stream ended unexpectedly")
            .expect("WS error");

        let text = msg.into_text().expect("not text");
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["type"], "Heartbeat");

        let _ = ws.close(None).await;
        server.abort();
    }

    #[tokio::test]
    async fn ws_rate_limit_drops_excess_not_disconnect() {
        let (url, _core, server) = setup_ws_server().await;
        let (mut ws, _) = tokio_tungstenite::connect_async(&url)
            .await
            .expect("WS connect failed");

        // Skip Welcome
        let _ = ws.next().await;

        // Send 15 messages rapidly — rate limit is 10/sec.
        // Excess messages are silently dropped, connection stays alive.
        for i in 0..15 {
            let pong = serde_json::json!({"type": "Pong"}).to_string();
            ws.send(tungstenite::Message::Text(pong.into()))
                .await
                .unwrap_or_else(|_| panic!("Send {i} failed — should stay alive during rate limit"));
        }

        // Connection should still be alive — wait for Heartbeat
        let msg = tokio::time::timeout(Duration::from_secs(35), ws.next())
            .await
            .expect("timeout — connection should survive rate-limited burst")
            .expect("stream ended unexpectedly")
            .expect("WS error");

        let text = msg.into_text().expect("not text");
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["type"], "Heartbeat");

        let _ = ws.close(None).await;
        server.abort();
    }
}
