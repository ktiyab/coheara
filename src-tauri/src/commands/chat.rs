//! L3-03 Chat Interface — Tauri IPC commands.
//!
//! Commands:
//! - `start_conversation`: create a new conversation
//! - `send_chat_message`: save patient message + stream RAG response via events
//! - `get_conversation_messages`: load messages for a conversation
//! - `list_conversations`: list all conversations with summaries
//! - `delete_conversation`: remove conversation and all messages
//! - `set_message_feedback`: save helpful/not_helpful/clear
//! - `get_prompt_suggestions`: contextual prompt suggestions

use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::chat::{
    self, generate_title, update_conversation_title, ChatStreamEvent, ConversationSummary,
    PromptSuggestion, StreamChunkPayload,
};
use crate::db::sqlite::open_database;
use crate::models::enums::{MessageFeedback, MessageRole};
use crate::pipeline::rag::conversation::ConversationManager;

use super::state::AppState;

/// Start a new conversation. Returns the conversation ID.
#[tauri::command]
pub fn start_conversation(state: State<'_, AppState>) -> Result<String, String> {
    let guard = state
        .active_session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;
    let manager = ConversationManager::new(&conn);
    let conv_id = manager
        .start(Some("New conversation"))
        .map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(conv_id.to_string())
}

/// Send a patient message and stream the response via Tauri events.
///
/// Flow:
/// 1. Saves the patient message in the database
/// 2. Updates the conversation title from the first message
/// 3. Streams the RAG response via `chat-stream` events
///
/// Currently: RAG pipeline is not wired into AppState, so a placeholder
/// response is emitted. When the pipeline is integrated, this command
/// will call `RagPipeline::query()` and stream tokens.
#[tauri::command]
pub fn send_chat_message(
    conversation_id: String,
    text: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let guard = state
        .active_session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let conv_uuid =
        Uuid::parse_str(&conversation_id).map_err(|e| format!("Invalid conversation ID: {e}"))?;

    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;
    let manager = ConversationManager::new(&conn);

    // 1. Save patient message
    manager
        .add_patient_message(conv_uuid, &text)
        .map_err(|e| e.to_string())?;

    // 2. Update conversation title from first message if still default
    let history = manager.get_history(conv_uuid).map_err(|e| e.to_string())?;
    let patient_messages: Vec<_> = history
        .iter()
        .filter(|m| m.role == MessageRole::Patient)
        .collect();
    if patient_messages.len() == 1 {
        let title = generate_title(&text);
        let _ = update_conversation_title(&conn, &conversation_id, &title);
    }

    // 3. Stream response (placeholder until RAG pipeline is wired into AppState)
    //    When integrated: call rag_pipeline.query() → stream tokens → safety filter → emit Done
    let placeholder = "I'm not connected to the AI assistant yet. \
        Please make sure Ollama is running with the MedGemma model to enable chat responses.";

    let _ = app.emit(
        "chat-stream",
        &ChatStreamEvent {
            conversation_id: conversation_id.clone(),
            chunk: StreamChunkPayload::Token {
                text: placeholder.to_string(),
            },
        },
    );

    let _ = app.emit(
        "chat-stream",
        &ChatStreamEvent {
            conversation_id: conversation_id.clone(),
            chunk: StreamChunkPayload::Done {
                full_text: placeholder.to_string(),
                confidence: 0.0,
                boundary_check: "Understanding".to_string(),
            },
        },
    );

    // Save placeholder response
    manager
        .add_response(conv_uuid, placeholder, None, 0.0)
        .map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(())
}

/// Get all messages for a conversation, ordered by timestamp ASC.
#[tauri::command]
pub fn get_conversation_messages(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<MessageView>, String> {
    let guard = state
        .active_session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let conv_uuid =
        Uuid::parse_str(&conversation_id).map_err(|e| format!("Invalid conversation ID: {e}"))?;

    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;
    let manager = ConversationManager::new(&conn);
    let messages = manager
        .get_history(conv_uuid)
        .map_err(|e| e.to_string())?;

    let views: Vec<MessageView> = messages.into_iter().map(MessageView::from).collect();

    state.update_activity();
    Ok(views)
}

/// List all conversations with summaries, ordered by last_message_at DESC.
#[tauri::command]
pub fn list_conversations(state: State<'_, AppState>) -> Result<Vec<ConversationSummary>, String> {
    let guard = state
        .active_session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;
    let summaries = chat::list_conversation_summaries(&conn).map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(summaries)
}

/// Delete a conversation and all its messages (CASCADE).
#[tauri::command]
pub fn delete_conversation(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let guard = state
        .active_session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    Uuid::parse_str(&conversation_id).map_err(|e| format!("Invalid conversation ID: {e}"))?;

    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;
    chat::delete_conversation(&conn, &conversation_id).map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(())
}

/// Set or clear feedback for a message.
/// `feedback` should be "Helpful", "NotHelpful", or null to clear.
#[tauri::command]
pub fn set_message_feedback(
    message_id: String,
    feedback: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let guard = state
        .active_session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let msg_uuid = Uuid::parse_str(&message_id).map_err(|e| format!("Invalid message ID: {e}"))?;

    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;

    match feedback.as_deref() {
        Some("Helpful") => {
            let manager = ConversationManager::new(&conn);
            manager
                .set_feedback(msg_uuid, MessageFeedback::Helpful)
                .map_err(|e| e.to_string())?;
        }
        Some("NotHelpful") => {
            let manager = ConversationManager::new(&conn);
            manager
                .set_feedback(msg_uuid, MessageFeedback::NotHelpful)
                .map_err(|e| e.to_string())?;
        }
        _ => {
            chat::clear_message_feedback(&conn, &message_id).map_err(|e| e.to_string())?;
        }
    }

    state.update_activity();
    Ok(())
}

/// Get prompt suggestions based on the patient's data.
#[tauri::command]
pub fn get_prompt_suggestions(
    state: State<'_, AppState>,
) -> Result<Vec<PromptSuggestion>, String> {
    let guard = state
        .active_session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;
    let suggestions = chat::get_contextual_suggestions(&conn).map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(suggestions)
}

// ═══════════════════════════════════════════
// Message view for frontend serialization
// ═══════════════════════════════════════════

/// Frontend-friendly message representation.
/// Converts NaiveDateTime to String, MessageRole/MessageFeedback to String.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MessageView {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub source_chunks: Option<String>,
    pub confidence: Option<f32>,
    pub feedback: Option<String>,
}

impl From<crate::models::Message> for MessageView {
    fn from(m: crate::models::Message) -> Self {
        MessageView {
            id: m.id.to_string(),
            conversation_id: m.conversation_id.to_string(),
            role: m.role.as_str().to_string(),
            content: m.content,
            timestamp: m.timestamp.to_string(),
            source_chunks: m.source_chunks,
            confidence: m.confidence,
            feedback: m.feedback.map(|f| match f {
                MessageFeedback::Helpful => "Helpful".to_string(),
                MessageFeedback::NotHelpful => "NotHelpful".to_string(),
            }),
        }
    }
}
