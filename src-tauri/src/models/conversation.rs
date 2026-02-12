use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::{MessageFeedback, MessageRole};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: Uuid,
    pub started_at: NaiveDateTime,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: NaiveDateTime,
    pub source_chunks: Option<String>,
    pub confidence: Option<f32>,
    pub feedback: Option<MessageFeedback>,
}
