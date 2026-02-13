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

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::chat::{
    self, generate_title, update_conversation_title, ChatStreamEvent, CitationView,
    ConversationSummary, PromptSuggestion, StreamChunkPayload,
};
use crate::core_state::CoreState;
use crate::models::enums::{MessageFeedback, MessageRole};
use crate::pipeline::rag::conversation::ConversationManager;
use crate::pipeline::rag::orchestrator::DocumentRagPipeline;
use crate::pipeline::rag::types::{PatientQuery, RagResponse};
use crate::pipeline::safety::orchestrator::SafetyFilterImpl;
use crate::pipeline::safety::types::{FilterOutcome, SafetyFilter};

/// Start a new conversation. Returns the conversation ID.
#[tauri::command]
pub fn start_conversation(state: State<'_, Arc<CoreState>>) -> Result<String, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
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
/// 1. Sanitize patient input (L2-02 safety filter)
/// 2. Save the patient message in the database
/// 3. Update conversation title from first message
/// 4. Try RAG pipeline if Ollama is available; otherwise emit placeholder
/// 5. Filter RAG response through safety layers before emitting
#[tauri::command]
pub fn send_chat_message(
    conversation_id: String,
    text: String,
    state: State<'_, Arc<CoreState>>,
    app: AppHandle,
) -> Result<(), String> {
    let db_path = state.db_path().map_err(|e| e.to_string())?;
    let conn = state.open_db().map_err(|e| e.to_string())?;

    let conv_uuid =
        Uuid::parse_str(&conversation_id).map_err(|e| format!("Invalid conversation ID: {e}"))?;

    let manager = ConversationManager::new(&conn);
    let safety = SafetyFilterImpl::new();

    // 1. Sanitize patient input (L2-02: remove injection patterns, control chars)
    let sanitized = safety
        .sanitize_input(&text)
        .map_err(|e| format!("Input sanitization failed: {e}"))?;

    if sanitized.was_modified {
        tracing::info!(
            modifications = sanitized.modifications.len(),
            "Patient input sanitized before processing"
        );
    }

    // 2. Save patient message (sanitized)
    manager
        .add_patient_message(conv_uuid, &sanitized.text)
        .map_err(|e| e.to_string())?;

    // 3. Update conversation title from first message if still default
    let history = manager.get_history(conv_uuid).map_err(|e| e.to_string())?;
    let patient_messages: Vec<_> = history
        .iter()
        .filter(|m| m.role == MessageRole::Patient)
        .collect();
    if patient_messages.len() == 1 {
        let title = generate_title(&sanitized.text);
        let _ = update_conversation_title(&conn, &conversation_id, &title);
    }

    // 4. Attempt RAG pipeline; fall back to placeholder if unavailable
    match try_rag_query(&sanitized.text, conv_uuid, &conn, &db_path) {
        Some(rag_response) => {
            // 5. Filter RAG response through safety layers
            emit_filtered_response(&app, &conversation_id, &rag_response, &safety, &manager, conv_uuid)?;
        }
        None => {
            emit_placeholder_response(&app, &conversation_id, &manager, conv_uuid)?;
        }
    }

    state.update_activity();
    Ok(())
}

/// Attempt to run the RAG pipeline. Returns `None` if AI services unavailable.
///
/// Uses production components:
/// - **LLM**: OllamaRagGenerator (MedGemma via local Ollama)
/// - **Embedder**: OnnxEmbedder when `onnx-embeddings` feature enabled + model present;
///   falls back to MockEmbedder (deterministic vectors — no real semantic search)
/// - **Vector store**: SqliteVectorStore (persistent, brute-force cosine similarity)
fn try_rag_query(
    query_text: &str,
    conversation_id: Uuid,
    conn: &rusqlite::Connection,
    db_path: &std::path::Path,
) -> Option<RagResponse> {
    use crate::pipeline::rag::ollama::OllamaRagGenerator;
    use crate::pipeline::storage::vectordb::SqliteVectorStore;

    // Check if Ollama is available with a MedGemma model
    let generator = OllamaRagGenerator::try_auto_detect()?;

    // Production vector store — persistent SQLite-backed chunk storage
    let vector_store = SqliteVectorStore::new(db_path.to_path_buf());

    // Production embedder — ONNX when available, deterministic mock otherwise
    let embedder: Box<dyn crate::pipeline::storage::types::EmbeddingModel> =
        build_embedder();

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
                "RAG pipeline generated response"
            );
            Some(response)
        }
        Err(e) => {
            tracing::warn!(error = %e, "RAG pipeline failed, falling back to placeholder");
            None
        }
    }
}

/// Build the best available embedding model.
///
/// When `onnx-embeddings` feature is enabled and model files are present,
/// loads the real all-MiniLM-L6-v2 ONNX model. Otherwise falls back to
/// MockEmbedder which produces deterministic vectors (structured data
/// retrieval still works via SQLite, only semantic search is degraded).
fn build_embedder() -> Box<dyn crate::pipeline::storage::types::EmbeddingModel> {
    #[cfg(feature = "onnx-embeddings")]
    {
        let model_dir = crate::config::embedding_model_dir();
        if model_dir.join("model.onnx").exists() && model_dir.join("tokenizer.json").exists() {
            match crate::pipeline::storage::embedder::OnnxEmbedder::load(&model_dir) {
                Ok(e) => {
                    tracing::info!(dir = %model_dir.display(), "ONNX embedder loaded");
                    return Box::new(e);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "ONNX embedder failed to load, using mock");
                }
            }
        } else {
            tracing::info!("ONNX model files not found at {}, using mock embedder", model_dir.display());
        }
    }

    #[cfg(not(feature = "onnx-embeddings"))]
    {
        tracing::debug!("onnx-embeddings feature not enabled, using mock embedder");
    }

    Box::new(crate::pipeline::storage::embedder::MockEmbedder::new())
}

/// Emit a RAG response filtered through safety layers.
fn emit_filtered_response(
    app: &AppHandle,
    conversation_id: &str,
    rag_response: &RagResponse,
    safety: &SafetyFilterImpl,
    manager: &ConversationManager<'_>,
    conv_uuid: Uuid,
) -> Result<(), String> {
    // Run safety filter on RAG output (L2-02: boundary, keyword, grounding checks)
    let filtered = safety
        .filter_response(rag_response)
        .map_err(|e| format!("Safety filter error: {e}"))?;

    let (display_text, confidence, boundary_str) = match &filtered.filter_outcome {
        FilterOutcome::Passed => (
            filtered.text.clone(),
            filtered.confidence,
            format!("{:?}", filtered.boundary_check),
        ),
        FilterOutcome::Rephrased { .. } => {
            tracing::info!("Safety filter rephrased RAG response");
            (
                filtered.text.clone(),
                filtered.confidence,
                format!("{:?}", filtered.boundary_check),
            )
        }
        FilterOutcome::Blocked { fallback_message, .. } => {
            tracing::warn!("Safety filter blocked RAG response");
            (fallback_message.clone(), 0.0, "OutOfBounds".to_string())
        }
    };

    // Emit citations
    for citation in &rag_response.citations {
        let _ = app.emit(
            "chat-stream",
            &ChatStreamEvent {
                conversation_id: conversation_id.to_string(),
                chunk: StreamChunkPayload::Citation {
                    citation: CitationView::from(citation.clone()),
                },
            },
        );
    }

    // Emit response text
    let _ = app.emit(
        "chat-stream",
        &ChatStreamEvent {
            conversation_id: conversation_id.to_string(),
            chunk: StreamChunkPayload::Token {
                text: display_text.clone(),
            },
        },
    );

    // Emit done
    let _ = app.emit(
        "chat-stream",
        &ChatStreamEvent {
            conversation_id: conversation_id.to_string(),
            chunk: StreamChunkPayload::Done {
                full_text: display_text.clone(),
                confidence,
                boundary_check: boundary_str,
            },
        },
    );

    // Persist the filtered response
    let source_chunks_json = if rag_response.citations.is_empty() {
        None
    } else {
        serde_json::to_string(
            &rag_response
                .citations
                .iter()
                .map(|c| c.document_id.to_string())
                .collect::<Vec<_>>(),
        )
        .ok()
    };

    manager
        .add_response(conv_uuid, &display_text, source_chunks_json.as_deref(), confidence)
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Emit a placeholder response when AI services are unavailable.
fn emit_placeholder_response(
    app: &AppHandle,
    conversation_id: &str,
    manager: &ConversationManager<'_>,
    conv_uuid: Uuid,
) -> Result<(), String> {
    let placeholder = "I'm not connected to the AI assistant yet. \
        Please make sure Ollama is running with the MedGemma model to enable chat responses.";

    let _ = app.emit(
        "chat-stream",
        &ChatStreamEvent {
            conversation_id: conversation_id.to_string(),
            chunk: StreamChunkPayload::Token {
                text: placeholder.to_string(),
            },
        },
    );

    let _ = app.emit(
        "chat-stream",
        &ChatStreamEvent {
            conversation_id: conversation_id.to_string(),
            chunk: StreamChunkPayload::Done {
                full_text: placeholder.to_string(),
                confidence: 0.0,
                boundary_check: "Understanding".to_string(),
            },
        },
    );

    manager
        .add_response(conv_uuid, placeholder, None, 0.0)
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Get all messages for a conversation, ordered by timestamp ASC.
#[tauri::command]
pub fn get_conversation_messages(
    conversation_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<MessageView>, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;

    let conv_uuid =
        Uuid::parse_str(&conversation_id).map_err(|e| format!("Invalid conversation ID: {e}"))?;

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
pub fn list_conversations(state: State<'_, Arc<CoreState>>) -> Result<Vec<ConversationSummary>, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let summaries = chat::list_conversation_summaries(&conn).map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(summaries)
}

/// Delete a conversation and all its messages (CASCADE).
#[tauri::command]
pub fn delete_conversation(
    conversation_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    Uuid::parse_str(&conversation_id).map_err(|e| format!("Invalid conversation ID: {e}"))?;

    let conn = state.open_db().map_err(|e| e.to_string())?;
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
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let msg_uuid = Uuid::parse_str(&message_id).map_err(|e| format!("Invalid message ID: {e}"))?;

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
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<PromptSuggestion>, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
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
