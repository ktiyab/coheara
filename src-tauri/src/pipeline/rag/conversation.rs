use chrono::Local;
use rusqlite::Connection;
use uuid::Uuid;

use super::RagError;
use crate::db::repository;
use crate::models::enums::{MessageFeedback, MessageRole};
use crate::models::{Conversation, Message};

/// Manages conversation lifecycle and message persistence.
pub struct ConversationManager<'a> {
    conn: &'a Connection,
}

impl<'a> ConversationManager<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Start a new conversation. Returns the conversation ID.
    pub fn start(&self, title: Option<&str>) -> Result<Uuid, RagError> {
        let conversation = Conversation {
            id: Uuid::new_v4(),
            started_at: Local::now().naive_local(),
            title: title.map(|t| t.to_string()),
        };
        repository::insert_conversation(self.conn, &conversation)?;
        Ok(conversation.id)
    }

    /// Add a patient message to an existing conversation.
    pub fn add_patient_message(
        &self,
        conversation_id: Uuid,
        text: &str,
    ) -> Result<Uuid, RagError> {
        self.ensure_conversation_exists(conversation_id)?;

        let msg = Message {
            id: Uuid::new_v4(),
            conversation_id,
            role: MessageRole::Patient,
            content: text.to_string(),
            timestamp: Local::now().naive_local(),
            source_chunks: None,
            confidence: None,
            feedback: None,
        };
        repository::insert_message(self.conn, &msg)?;
        Ok(msg.id)
    }

    /// Add a Coheara response with citations and confidence.
    pub fn add_response(
        &self,
        conversation_id: Uuid,
        text: &str,
        source_chunks_json: Option<&str>,
        confidence: f32,
    ) -> Result<Uuid, RagError> {
        self.ensure_conversation_exists(conversation_id)?;

        let msg = Message {
            id: Uuid::new_v4(),
            conversation_id,
            role: MessageRole::Coheara,
            content: text.to_string(),
            timestamp: Local::now().naive_local(),
            source_chunks: source_chunks_json.map(|s| s.to_string()),
            confidence: Some(confidence),
            feedback: None,
        };
        repository::insert_message(self.conn, &msg)?;
        Ok(msg.id)
    }

    /// Get recent messages for a conversation (ordered by timestamp).
    pub fn get_history(&self, conversation_id: Uuid) -> Result<Vec<Message>, RagError> {
        self.ensure_conversation_exists(conversation_id)?;
        let messages = repository::get_messages_by_conversation(self.conn, &conversation_id)?;
        Ok(messages)
    }

    /// Update feedback on a message.
    pub fn set_feedback(
        &self,
        message_id: Uuid,
        feedback: MessageFeedback,
    ) -> Result<(), RagError> {
        self.conn
            .execute(
                "UPDATE messages SET feedback = ?1 WHERE id = ?2",
                rusqlite::params![feedback.as_str(), message_id.to_string()],
            )
            .map_err(crate::db::DatabaseError::Sqlite)?;
        Ok(())
    }

    fn ensure_conversation_exists(&self, id: Uuid) -> Result<(), RagError> {
        let conv = repository::get_conversation(self.conn, &id)?;
        if conv.is_none() {
            return Err(RagError::ConversationNotFound(id));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;

    fn test_manager() -> (Connection, Uuid) {
        let conn = open_memory_database().unwrap();
        let manager = ConversationManager::new(&conn);
        let conv_id = manager.start(Some("Test conversation")).unwrap();
        (conn, conv_id)
    }

    #[test]
    fn start_conversation() {
        let conn = open_memory_database().unwrap();
        let manager = ConversationManager::new(&conn);
        let conv_id = manager.start(Some("My health questions")).unwrap();

        let conv = repository::get_conversation(&conn, &conv_id)
            .unwrap()
            .unwrap();
        assert_eq!(conv.title.as_deref(), Some("My health questions"));
    }

    #[test]
    fn add_and_retrieve_messages() {
        let (conn, conv_id) = test_manager();
        let manager = ConversationManager::new(&conn);

        let patient_id = manager
            .add_patient_message(conv_id, "What dose of metformin am I on?")
            .unwrap();
        let response_id = manager
            .add_response(
                conv_id,
                "Your documents show metformin 500mg twice daily.",
                Some(r#"["chunk1"]"#),
                0.85,
            )
            .unwrap();

        let history = manager.get_history(conv_id).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].id, patient_id);
        assert_eq!(history[0].role, MessageRole::Patient);
        assert_eq!(history[1].id, response_id);
        assert_eq!(history[1].role, MessageRole::Coheara);
        assert_eq!(history[1].confidence, Some(0.85));
    }

    #[test]
    fn message_to_nonexistent_conversation_fails() {
        let conn = open_memory_database().unwrap();
        let manager = ConversationManager::new(&conn);
        let fake_id = Uuid::new_v4();

        let result = manager.add_patient_message(fake_id, "Hello");
        assert!(result.is_err());
    }

    #[test]
    fn set_feedback_on_message() {
        let (conn, conv_id) = test_manager();
        let manager = ConversationManager::new(&conn);

        let msg_id = manager
            .add_response(conv_id, "Some response", None, 0.7)
            .unwrap();

        manager
            .set_feedback(msg_id, MessageFeedback::Helpful)
            .unwrap();

        let history = manager.get_history(conv_id).unwrap();
        assert_eq!(history[0].feedback, Some(MessageFeedback::Helpful));
    }

    #[test]
    fn empty_conversation_returns_empty_history() {
        let (conn, conv_id) = test_manager();
        let manager = ConversationManager::new(&conn);

        let history = manager.get_history(conv_id).unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn start_conversation_without_title() {
        let conn = open_memory_database().unwrap();
        let manager = ConversationManager::new(&conn);
        let conv_id = manager.start(None).unwrap();

        let conv = repository::get_conversation(&conn, &conv_id)
            .unwrap()
            .unwrap();
        assert!(conv.title.is_none());
    }
}
