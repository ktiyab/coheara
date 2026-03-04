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
    ConversationSummary, GuidelineCitationView, PromptSuggestion, StreamChunkPayload,
};
use crate::chat_queue::ChatQueueSnapshot;
use crate::core_state::CoreState;
use crate::crypto::profile::PatientDemographics;
use crate::invariants::InvariantRegistry;
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

/// Send a patient message — non-blocking enqueue pattern (CHAT-QUEUE-01).
///
/// Fast-path flow (~5ms):
/// 1. Sanitize patient input (L2-02 safety filter)
/// 2. Save the patient message in the database
/// 3. Update conversation title from first message
/// 4. Enqueue in ChatQueueService for deferred SLM processing
/// 5. Return queue_item_id immediately
///
/// The slow path (Butler acquire → RAG pipeline → streaming) runs in the
/// chat_queue_worker background task. Input re-enables immediately so the
/// user can queue multiple questions.
#[tauri::command]
pub async fn send_chat_message(
    conversation_id: String,
    text: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<String, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = state.open_db().map_err(|e| e.to_string())?;

        let conv_uuid =
            Uuid::parse_str(&conversation_id).map_err(|e| format!("Invalid conversation ID: {e}"))?;

        let manager = ConversationManager::new(&conn);
        // I18N-06: Use profile language for safety fallback messages
        let lang = state.get_profile_language();
        let safety = SafetyFilterImpl::with_language(&lang);

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

        // 2. Save patient message (sanitized) — returns the message UUID
        let patient_msg_id = manager
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

        // 4. Enqueue for deferred processing — worker handles Butler + RAG + streaming
        let queue_item_id = state.chat_queue().enqueue(
            conversation_id.clone(),
            patient_msg_id.to_string(),
            sanitized.text,
        );

        state.update_activity();
        Ok(queue_item_id)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

/// Attempt to run the RAG pipeline with streaming. Returns `None` if AI unavailable.
///
/// Tokens are streamed progressively via `token_tx` — the caller forwards them
/// as Tauri events so the frontend sees text building in real time.
///
/// Uses production components:
/// - **LLM**: OllamaRagGenerator (MedGemma via local Ollama)
/// - **Embedder**: OnnxEmbedder when `onnx-embeddings` feature enabled + model present;
///   falls back to MockEmbedder (deterministic vectors — no real semantic search)
/// - **Vector store**: SqliteVectorStore (persistent, brute-force cosine similarity)
///
/// CHAT-QUEUE-01: Made `pub(crate)` for use by `chat_queue_worker`.
pub(crate) fn try_rag_query(
    query_text: &str,
    conversation_id: Uuid,
    conn: &rusqlite::Connection,
    db_path: &std::path::Path,
    resolved_model: Option<&str>,
    db_key: Option<&[u8; 32]>,
    registry: &InvariantRegistry,
    lang: &str,
    demographics: Option<PatientDemographics>,
    token_tx: std::sync::mpsc::Sender<String>,
) -> Option<RagResponse> {
    use crate::pipeline::rag::ollama::OllamaRagGenerator;
    use crate::pipeline::storage::vectordb::SqliteVectorStore;

    // Use preference-resolved model if available (L6-04)
    let model_name = resolved_model?;
    let generator = OllamaRagGenerator::with_resolved_model(model_name.to_string())?;

    // Production vector store — persistent SQLite-backed chunk storage
    let vector_store = SqliteVectorStore::new(db_path.to_path_buf(), db_key.copied());

    // Production embedder — ONNX when available, deterministic mock otherwise
    let embedder: Box<dyn crate::pipeline::storage::types::EmbeddingModel> =
        build_embedder();

    let pipeline = DocumentRagPipeline::with_language(&generator, &embedder, &vector_store, conn, registry, lang)
        .with_demographics(demographics);
    let query = PatientQuery {
        text: query_text.to_string(),
        conversation_id,
        query_type: None,
    };

    match pipeline.generate_streaming(&query, token_tx) {
        Ok(response) => {
            tracing::info!(
                confidence = response.confidence,
                boundary = ?response.boundary_check,
                citations = response.citations.len(),
                "RAG pipeline generated streaming response"
            );
            Some(response)
        }
        Err(e) => {
            tracing::warn!(error = %e, "RAG pipeline failed, falling back to placeholder");
            None
        }
    }
}

/// Build the best available embedding model (delegates to shared builder).
pub(crate) fn build_embedder() -> Box<dyn crate::pipeline::storage::types::EmbeddingModel> {
    crate::pipeline::storage::embedder::build_embedder()
}

/// Emit citations and Done event for a safety-filtered RAG response.
///
/// Token events are already streamed progressively by the forwarder thread,
/// so this only emits Citations + Done (with final safety-filtered text).
///
/// CHAT-QUEUE-01: Made `pub(crate)` for use by `chat_queue_worker`.
pub(crate) fn emit_filtered_response(
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
        FilterOutcome::Blocked { fallback_message, .. } => {
            tracing::warn!("Safety filter blocked RAG response (boundary out of bounds)");
            (fallback_message.clone(), 0.0, "OutOfBounds".to_string())
        }
    };

    // Emit document citations
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

    // ME-03: Emit guideline citations (deterministic, from clinical insights)
    if !rag_response.guideline_citations.is_empty() {
        let _ = app.emit(
            "chat-stream",
            &ChatStreamEvent {
                conversation_id: conversation_id.to_string(),
                chunk: StreamChunkPayload::GuidelineCitations {
                    citations: rag_response
                        .guideline_citations
                        .iter()
                        .cloned()
                        .map(GuidelineCitationView::from)
                        .collect(),
                },
            },
        );
    }

    // Emit Done with final safety-filtered text (replaces streamed tokens)
    let _ = app.emit(
        "chat-stream",
        &ChatStreamEvent {
            conversation_id: conversation_id.to_string(),
            chunk: StreamChunkPayload::Done {
                full_text: display_text.clone(),
                confidence,
                boundary_check: boundary_str,
                grounding: format!("{:?}", filtered.grounding),
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
/// I18N-19: Localized based on user's language preference.
///
/// CHAT-QUEUE-01: Made `pub(crate)` for use by `chat_queue_worker`.
pub(crate) fn emit_placeholder_response(
    app: &AppHandle,
    conversation_id: &str,
    manager: &ConversationManager<'_>,
    conv_uuid: Uuid,
    lang: &str,
) -> Result<(), String> {
    let placeholder = match lang {
        "fr" => "Je ne suis pas encore connecté à l'assistant IA. \
            Veuillez vous assurer qu'Ollama fonctionne avec le modèle MedGemma pour activer les réponses.",
        "de" => "Ich bin noch nicht mit dem KI-Assistenten verbunden. \
            Bitte stellen Sie sicher, dass Ollama mit dem MedGemma-Modell läuft, um Chat-Antworten zu ermöglichen.",
        _ => "I'm not connected to the AI assistant yet. \
            Please make sure Ollama is running with the MedGemma model to enable chat responses.",
    };

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
                grounding: "None".to_string(),
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
/// LP-05: Uses SuggestionScorer with 5 SignalProviders for ranked, personalized suggestions.
#[tauri::command]
pub fn get_prompt_suggestions(
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<PromptSuggestion>, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let scorer = crate::suggestions::SuggestionScorer::new();
    let suggestions = scorer.score(&conn, 6).map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(suggestions)
}

// ═══════════════════════════════════════════
// CHAT-QUEUE-01: Queue IPC commands
// ═══════════════════════════════════════════

/// Get a snapshot of the entire chat queue (all items, processing flag).
#[tauri::command]
pub fn get_chat_queue(
    state: State<'_, Arc<CoreState>>,
) -> Result<ChatQueueSnapshot, String> {
    Ok(state.chat_queue().snapshot())
}

/// Get pending chat queue items for a specific conversation.
#[tauri::command]
pub fn get_chat_queue_for_conversation(
    conversation_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<crate::chat_queue::ChatQueueItem>, String> {
    Ok(state.chat_queue().pending_for_conversation(&conversation_id))
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
