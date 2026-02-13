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

    // Send Welcome message
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
        })
        .await;

    // Flush pending alerts (queued while device was disconnected)
    if let Ok(mut devices) = core.write_devices() {
        devices.flush_pending(&device_id);
    }

    // Main receive + heartbeat loop
    let session_start = Instant::now();
    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
    heartbeat.tick().await; // Consume initial immediate tick
    let mut missed_heartbeats: u32 = 0;
    let mut expiry_warned = false;
    let mut incoming_times: VecDeque<Instant> = VecDeque::new();

    loop {
        tokio::select! {
            msg = ws_stream.next() => {
                match msg {
                    Some(Ok(Message::Text(ref text))) => {
                        if !check_incoming_rate(&mut incoming_times) {
                            continue;
                        }
                        if let Ok(incoming) = serde_json::from_str::<WsIncoming>(text) {
                            match incoming {
                                WsIncoming::Pong {} => {
                                    missed_heartbeats = 0;
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
                let elapsed = session_start.elapsed();

                // Session max (1 hour)
                if elapsed >= SESSION_MAX {
                    let _ = tx.send(WsOutgoing::SessionExpiring {
                        seconds_remaining: 0,
                    }).await;
                    break;
                }

                // Expiry warning (60s before)
                if !expiry_warned && elapsed >= SESSION_MAX - EXPIRY_WARNING {
                    let remaining = (SESSION_MAX - elapsed).as_secs() as u32;
                    let _ = tx.send(WsOutgoing::SessionExpiring {
                        seconds_remaining: remaining,
                    }).await;
                    expiry_warned = true;
                }

                // Missed heartbeats → disconnect
                if missed_heartbeats >= MAX_MISSED_HEARTBEATS {
                    tracing::info!(
                        device_id = %device_id,
                        "{MAX_MISSED_HEARTBEATS} missed heartbeats, disconnecting"
                    );
                    break;
                }

                // Send heartbeat
                let _ = tx.send(WsOutgoing::Heartbeat {
                    server_time: chrono::Utc::now().to_rfc3339(),
                }).await;
                missed_heartbeats += 1;
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
            message: _,
        } => {
            // TODO (M1-02): Forward to RAG pipeline, stream tokens back via tx
            let conv_id =
                conversation_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let _ = tx
                .send(WsOutgoing::ChatToken {
                    conversation_id: conv_id.clone(),
                    token: "Chat via WebSocket coming in M1-02".to_string(),
                })
                .await;
            let _ = tx
                .send(WsOutgoing::ChatComplete {
                    conversation_id: conv_id,
                    citations: vec![],
                })
                .await;
        }
        WsIncoming::ChatFeedback {
            conversation_id,
            message_id,
            helpful,
        } => {
            core.log_access(
                crate::core_state::AccessSource::MobileDevice {
                    device_id: device_id.to_string(),
                },
                if helpful {
                    "chat_feedback_positive"
                } else {
                    "chat_feedback_negative"
                },
                &format!("conversation:{conversation_id}:message:{message_id}"),
            );
        }
        _ => {} // Ready and Pong handled in main loop
    }
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
}
