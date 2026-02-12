//! L3-03 Chat Interface — types, helpers, and repository functions.
//!
//! Builds on top of:
//! - `models::Conversation` / `models::Message` (data structs)
//! - `pipeline::rag::conversation::ConversationManager` (CRUD lifecycle)
//! - `db::repository` (low-level insert/query)
//!
//! This module adds:
//! - Frontend-specific types (ConversationSummary, CitationView, streaming events)
//! - Derived queries (conversation list with message counts, contextual suggestions)
//! - Title generation helper

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::db::DatabaseError;

// ═══════════════════════════════════════════
// Frontend-facing types
// ═══════════════════════════════════════════

/// Conversation summary for the conversation list sidebar.
/// Fields derived via JOIN since conversations table stores only id/started_at/title.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub last_message_at: String,
    pub message_count: u32,
    pub last_message_preview: String,
}

/// Citation as displayed in the frontend (String IDs for JS interop).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationView {
    pub document_id: String,
    pub document_title: String,
    pub document_date: Option<String>,
    pub professional_name: Option<String>,
    pub chunk_text: String,
    pub relevance_score: f32,
}

/// Payload emitted via Tauri event during streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatStreamEvent {
    pub conversation_id: String,
    pub chunk: StreamChunkPayload,
}

/// A single streaming chunk sent to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamChunkPayload {
    Token { text: String },
    Citation { citation: CitationView },
    Done {
        full_text: String,
        confidence: f32,
        boundary_check: String,
    },
    Error { message: String },
}

/// Prompt suggestion for empty state / new conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptSuggestion {
    pub text: String,
    pub category: String,
}

// ═══════════════════════════════════════════
// Title generation
// ═══════════════════════════════════════════

/// Generate a conversation title from the first patient message.
/// Truncates at 50 characters with "..." if longer, handling UTF-8 correctly.
pub fn generate_title(first_message: &str) -> String {
    let trimmed = first_message.trim();
    if trimmed.is_empty() {
        return "New conversation".to_string();
    }

    // Find the byte position at or just before the 50th character
    let boundary = trimmed
        .char_indices()
        .take_while(|(i, _)| *i < 50)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(trimmed.len());

    if boundary >= trimmed.len() {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..boundary])
    }
}

// ═══════════════════════════════════════════
// Prompt suggestions
// ═══════════════════════════════════════════

/// Default prompt suggestions for empty conversations.
pub fn default_prompt_suggestions() -> Vec<PromptSuggestion> {
    vec![
        PromptSuggestion {
            text: "What medications am I currently taking?".into(),
            category: "medications".into(),
        },
        PromptSuggestion {
            text: "Summarize my latest lab results".into(),
            category: "labs".into(),
        },
        PromptSuggestion {
            text: "Are there any interactions between my medications?".into(),
            category: "medications".into(),
        },
        PromptSuggestion {
            text: "What should I ask my doctor at my next visit?".into(),
            category: "appointments".into(),
        },
        PromptSuggestion {
            text: "Explain my diagnosis in simple terms".into(),
            category: "general".into(),
        },
        PromptSuggestion {
            text: "What changed since my last appointment?".into(),
            category: "general".into(),
        },
    ]
}

/// Get prompt suggestions contextual to the patient's data.
/// Replaces generic defaults with contextual suggestions when data exists.
/// Returns at most 6 suggestions.
pub fn get_contextual_suggestions(conn: &Connection) -> Result<Vec<PromptSuggestion>, DatabaseError> {
    let mut suggestions = default_prompt_suggestions();

    let has_meds: bool = conn
        .query_row("SELECT COUNT(*) > 0 FROM medications", [], |row| row.get(0))
        .unwrap_or(false);

    let has_labs: bool = conn
        .query_row("SELECT COUNT(*) > 0 FROM lab_results", [], |row| row.get(0))
        .unwrap_or(false);

    // Replace generic suggestions with contextual ones (from the end)
    if has_meds && suggestions.len() >= 6 {
        suggestions[5] = PromptSuggestion {
            text: "Do any of my medications have common side effects?".into(),
            category: "medications".into(),
        };
    }
    if has_labs && suggestions.len() >= 5 {
        suggestions[4] = PromptSuggestion {
            text: "Are any of my lab values outside the normal range?".into(),
            category: "labs".into(),
        };
    }

    Ok(suggestions)
}

// ═══════════════════════════════════════════
// Repository functions
// ═══════════════════════════════════════════

/// List all conversations with derived summary fields.
/// Conversations are per-profile (each profile has its own SQLite DB).
pub fn list_conversation_summaries(
    conn: &Connection,
) -> Result<Vec<ConversationSummary>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT
            c.id,
            COALESCE(c.title, 'New conversation') AS title,
            COALESCE(MAX(m.timestamp), c.started_at) AS last_message_at,
            COUNT(m.id) AS message_count,
            COALESCE(
                (SELECT SUBSTR(m2.content, 1, 80) FROM messages m2
                 WHERE m2.conversation_id = c.id
                 ORDER BY m2.timestamp DESC LIMIT 1),
                ''
            ) AS last_message_preview
         FROM conversations c
         LEFT JOIN messages m ON m.conversation_id = c.id
         GROUP BY c.id
         ORDER BY last_message_at DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(ConversationSummary {
            id: row.get(0)?,
            title: row.get(1)?,
            last_message_at: row.get(2)?,
            message_count: row.get::<_, i64>(3)? as u32,
            last_message_preview: row.get(4)?,
        })
    })?;

    let mut summaries = Vec::new();
    for row in rows {
        summaries.push(row?);
    }
    Ok(summaries)
}

/// Delete a conversation and all its messages (CASCADE).
pub fn delete_conversation(conn: &Connection, conversation_id: &str) -> Result<bool, DatabaseError> {
    let rows_affected = conn.execute(
        "DELETE FROM conversations WHERE id = ?1",
        params![conversation_id],
    )?;
    Ok(rows_affected > 0)
}

/// Clear feedback on a message (set to NULL).
pub fn clear_message_feedback(conn: &Connection, message_id: &str) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE messages SET feedback = NULL WHERE id = ?1",
        params![message_id],
    )?;
    Ok(())
}

/// Update the title of a conversation.
pub fn update_conversation_title(
    conn: &Connection,
    conversation_id: &str,
    title: &str,
) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE conversations SET title = ?1 WHERE id = ?2",
        params![title, conversation_id],
    )?;
    Ok(())
}

// ═══════════════════════════════════════════
// Citation conversion
// ═══════════════════════════════════════════

impl From<crate::pipeline::rag::types::Citation> for CitationView {
    fn from(c: crate::pipeline::rag::types::Citation) -> Self {
        CitationView {
            document_id: c.document_id.to_string(),
            document_title: c.document_title,
            document_date: c.document_date,
            professional_name: c.professional_name,
            chunk_text: c.chunk_text,
            relevance_score: c.relevance_score,
        }
    }
}

// ═══════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;
    use crate::pipeline::rag::conversation::ConversationManager;
    use uuid::Uuid;

    // ── Title generation ──

    #[test]
    fn generate_title_short_message() {
        assert_eq!(generate_title("What is metformin?"), "What is metformin?");
    }

    #[test]
    fn generate_title_exactly_50_chars() {
        let msg = "A".repeat(50);
        assert_eq!(generate_title(&msg), msg);
    }

    #[test]
    fn generate_title_long_message_truncated() {
        let msg = "A".repeat(80);
        let title = generate_title(&msg);
        assert!(title.ends_with("..."));
        assert!(title.len() <= 53); // 50 chars + "..."
    }

    #[test]
    fn generate_title_unicode_safe() {
        // 日本語 is 3 bytes per char — ensure we don't split mid-character
        let msg = "日本語のテキストを書いています。これは五十文字を超えるテキストです。";
        let title = generate_title(msg);
        assert!(title.ends_with("..."));
        // Should be valid UTF-8
        assert!(title.is_char_boundary(title.len() - 3));
    }

    #[test]
    fn generate_title_whitespace_trimmed() {
        assert_eq!(generate_title("  Hello world  "), "Hello world");
    }

    #[test]
    fn generate_title_empty_message() {
        assert_eq!(generate_title(""), "New conversation");
        assert_eq!(generate_title("   "), "New conversation");
    }

    // ── Prompt suggestions ──

    #[test]
    fn default_suggestions_returns_six() {
        let suggestions = default_prompt_suggestions();
        assert_eq!(suggestions.len(), 6);
        assert!(suggestions.iter().all(|s| !s.text.is_empty()));
        assert!(suggestions.iter().all(|s| !s.category.is_empty()));
    }

    #[test]
    fn contextual_suggestions_no_data() {
        let conn = open_memory_database().unwrap();
        let suggestions = get_contextual_suggestions(&conn).unwrap();
        // No medications or labs → defaults only, still capped at 6
        assert_eq!(suggestions.len(), 6);
    }

    #[test]
    fn contextual_suggestions_with_medications() {
        let conn = open_memory_database().unwrap();

        // Insert a document and medication to trigger contextual suggestion
        let doc_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO documents (id, type, title, source_file, ingestion_date, verified)
             VALUES (?1, 'prescription', 'Test Rx', 'test.pdf', datetime('now'), 0)",
            params![doc_id],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO medications (id, document_id, generic_name, dose, frequency, frequency_type, status)
             VALUES (?1, ?2, 'Metformin', '500mg', 'twice daily', 'scheduled', 'active')",
            params![Uuid::new_v4().to_string(), doc_id],
        )
        .unwrap();

        let suggestions = get_contextual_suggestions(&conn).unwrap();
        assert_eq!(suggestions.len(), 6); // 6 defaults, +1 contextual → truncated to 6
        // The last suggestion should be the contextual one (replaces last default)
        assert!(suggestions
            .iter()
            .any(|s| s.text.contains("side effects")));
    }

    // ── Conversation summaries ──

    #[test]
    fn list_summaries_empty() {
        let conn = open_memory_database().unwrap();
        let summaries = list_conversation_summaries(&conn).unwrap();
        assert!(summaries.is_empty());
    }

    #[test]
    fn list_summaries_with_messages() {
        let conn = open_memory_database().unwrap();
        let manager = ConversationManager::new(&conn);

        let conv_id = manager.start(Some("My health questions")).unwrap();
        manager
            .add_patient_message(conv_id, "What dose of metformin?")
            .unwrap();
        manager
            .add_response(conv_id, "Your dose is 500mg twice daily.", None, 0.85)
            .unwrap();

        let summaries = list_conversation_summaries(&conn).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].title, "My health questions");
        assert_eq!(summaries[0].message_count, 2);
        assert_eq!(
            summaries[0].last_message_preview,
            "Your dose is 500mg twice daily."
        );
    }

    #[test]
    fn list_summaries_ordered_by_last_message() {
        let conn = open_memory_database().unwrap();
        let manager = ConversationManager::new(&conn);

        let old_id = manager.start(Some("Old conversation")).unwrap();
        manager
            .add_patient_message(old_id, "Old question")
            .unwrap();

        // Small delay to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(10));

        let new_id = manager.start(Some("New conversation")).unwrap();
        manager
            .add_patient_message(new_id, "New question")
            .unwrap();

        let summaries = list_conversation_summaries(&conn).unwrap();
        assert_eq!(summaries.len(), 2);
        // Most recent first
        assert_eq!(summaries[0].title, "New conversation");
        assert_eq!(summaries[1].title, "Old conversation");
    }

    // ── Delete conversation ──

    #[test]
    fn delete_conversation_cascades_messages() {
        let conn = open_memory_database().unwrap();
        let manager = ConversationManager::new(&conn);

        let conv_id = manager.start(Some("To delete")).unwrap();
        manager
            .add_patient_message(conv_id, "Hello")
            .unwrap();

        let deleted = delete_conversation(&conn, &conv_id.to_string()).unwrap();
        assert!(deleted);

        // Verify conversation is gone
        let summaries = list_conversation_summaries(&conn).unwrap();
        assert!(summaries.is_empty());

        // Verify messages are gone (CASCADE)
        let msg_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
                params![conv_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(msg_count, 0);
    }

    #[test]
    fn delete_nonexistent_conversation() {
        let conn = open_memory_database().unwrap();
        let deleted = delete_conversation(&conn, &Uuid::new_v4().to_string()).unwrap();
        assert!(!deleted);
    }

    // ── Feedback ──

    #[test]
    fn clear_feedback_sets_null() {
        let conn = open_memory_database().unwrap();
        let manager = ConversationManager::new(&conn);

        let conv_id = manager.start(Some("Test")).unwrap();
        let msg_id = manager
            .add_response(conv_id, "Response", None, 0.8)
            .unwrap();

        // Set feedback first
        manager
            .set_feedback(msg_id, crate::models::enums::MessageFeedback::Helpful)
            .unwrap();

        // Clear it
        clear_message_feedback(&conn, &msg_id.to_string()).unwrap();

        // Verify it's null
        let history = manager.get_history(conv_id).unwrap();
        assert_eq!(history[0].feedback, None);
    }

    // ── Citation conversion ──

    #[test]
    fn citation_to_citation_view() {
        let citation = crate::pipeline::rag::types::Citation {
            document_id: Uuid::new_v4(),
            document_title: "Prescription".to_string(),
            document_date: Some("2024-01-15".to_string()),
            professional_name: Some("Dr. Chen".to_string()),
            chunk_text: "Metformin 500mg twice daily".to_string(),
            relevance_score: 0.92,
        };

        let view: CitationView = citation.clone().into();
        assert_eq!(view.document_id, citation.document_id.to_string());
        assert_eq!(view.document_title, "Prescription");
        assert_eq!(view.professional_name, Some("Dr. Chen".to_string()));
        assert!((view.relevance_score - 0.92).abs() < f32::EPSILON);
    }
}
