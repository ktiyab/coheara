//! M0-01: Chat endpoints.
//!
//! Three endpoints:
//! - `POST /api/chat/send` — send a message (returns ack, not streaming)
//! - `GET /api/chat/conversations` — list recent conversations
//! - `GET /api/chat/conversations/:id` — full conversation messages

use axum::extract::{Path, State};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::error::ApiError;
use crate::api::types::{ApiContext, DeviceContext};
use crate::chat;

#[derive(Deserialize)]
pub struct ChatSendRequest {
    pub conversation_id: Option<String>,
    pub message: String,
}

#[derive(Serialize)]
pub struct ChatAckResponse {
    pub conversation_id: String,
    pub message_id: String,
    pub disclaimer: &'static str,
}

/// `POST /api/chat/send` — send a chat message.
///
/// For M0-01, returns an ack with conversation_id. The actual AI
/// response will be delivered via M0-03 WebSocket in a future phase.
/// For now, the message is stored and processing is deferred.
pub async fn send(
    State(ctx): State<ApiContext>,
    Extension(_device): Extension<DeviceContext>,
    Json(req): Json<ChatSendRequest>,
) -> Result<Json<ChatAckResponse>, ApiError> {
    if req.message.trim().is_empty() {
        return Err(ApiError::BadRequest("Message cannot be empty".into()));
    }
    if req.message.len() > 2000 {
        return Err(ApiError::BadRequest("Message too long (max 2000 chars)".into()));
    }

    let conn = ctx.core.open_db()?;

    // Create or reuse conversation
    let conversation_id = match req.conversation_id {
        Some(id) => id,
        None => {
            let conv_id = uuid::Uuid::new_v4().to_string();
            let title = chat::generate_title(&req.message);
            conn.execute(
                "INSERT INTO conversations (id, title, started_at) VALUES (?1, ?2, datetime('now'))",
                rusqlite::params![conv_id, title],
            )
            .map_err(|e| ApiError::Internal(format!("Failed to create conversation: {e}")))?;
            conv_id
        }
    };

    // Store the patient message
    let message_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, timestamp)
         VALUES (?1, ?2, 'patient', ?3, datetime('now'))",
        rusqlite::params![message_id, conversation_id, req.message.trim()],
    )
    .map_err(|e| ApiError::Internal(format!("Failed to store message: {e}")))?;

    ctx.core.update_activity();

    Ok(Json(ChatAckResponse {
        conversation_id,
        message_id,
        disclaimer: "This helps you understand your records. Always confirm with your healthcare team.",
    }))
}

#[derive(Serialize)]
pub struct ConversationsResponse {
    pub profile_name: String,
    pub conversations: Vec<chat::ConversationSummary>,
}

/// `GET /api/chat/conversations` — list recent conversations.
pub async fn conversations(
    State(ctx): State<ApiContext>,
    Extension(_device): Extension<DeviceContext>,
) -> Result<Json<ConversationsResponse>, ApiError> {
    let profile_name = {
        let guard = ctx.core.read_session()?;
        let session = guard.as_ref().ok_or(ApiError::NoActiveProfile)?;
        session.profile_name.clone()
    };
    let conn = ctx.core.open_db()?;
    let convs = chat::list_conversation_summaries(&conn).map_err(ApiError::from)?;
    ctx.core.update_activity();

    Ok(Json(ConversationsResponse {
        profile_name,
        conversations: convs,
    }))
}

#[derive(Serialize)]
pub struct ConversationMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct ConversationDetailResponse {
    pub profile_name: String,
    pub conversation_id: String,
    pub messages: Vec<ConversationMessage>,
}

/// `GET /api/chat/conversations/:id` — full conversation messages.
pub async fn conversation(
    State(ctx): State<ApiContext>,
    Extension(_device): Extension<DeviceContext>,
    Path(conversation_id): Path<String>,
) -> Result<Json<ConversationDetailResponse>, ApiError> {
    let profile_name = {
        let guard = ctx.core.read_session()?;
        let session = guard.as_ref().ok_or(ApiError::NoActiveProfile)?;
        session.profile_name.clone()
    };
    let conn = ctx.core.open_db()?;

    let mut stmt = conn
        .prepare(
            "SELECT id, role, content, timestamp FROM messages
             WHERE conversation_id = ?1 ORDER BY timestamp ASC",
        )
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let messages = stmt
        .query_map(rusqlite::params![conversation_id], |row| {
            Ok(ConversationMessage {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();

    if messages.is_empty() {
        // Check if conversation exists
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
                rusqlite::params![conversation_id],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if !exists {
            return Err(ApiError::NotFound("Conversation not found".into()));
        }
    }

    ctx.core.update_activity();

    Ok(Json(ConversationDetailResponse {
        profile_name,
        conversation_id,
        messages,
    }))
}
