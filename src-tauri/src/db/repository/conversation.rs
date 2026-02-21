use std::str::FromStr;

use chrono::NaiveDateTime;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::DatabaseError;
use crate::models::*;
use crate::models::enums::*;

pub fn insert_conversation(conn: &Connection, conv: &Conversation) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO conversations (id, started_at, title) VALUES (?1, ?2, ?3)",
        params![
            conv.id.to_string(),
            conv.started_at.to_string(),
            conv.title,
        ],
    )?;
    Ok(())
}

pub fn get_conversation(conn: &Connection, id: &Uuid) -> Result<Option<Conversation>, DatabaseError> {
    let result = conn.query_row(
        "SELECT id, started_at, title FROM conversations WHERE id = ?1",
        params![id.to_string()],
        |row| {
            Ok(Conversation {
                id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                started_at: NaiveDateTime::parse_from_str(
                    &row.get::<_, String>(1)?,
                    "%Y-%m-%d %H:%M:%S",
                )
                .unwrap_or_default(),
                title: row.get(2)?,
            })
        },
    );

    match result {
        Ok(conv) => Ok(Some(conv)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn insert_message(conn: &Connection, msg: &Message) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, timestamp, source_chunks, confidence, feedback)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            msg.id.to_string(),
            msg.conversation_id.to_string(),
            msg.role.as_str(),
            msg.content,
            msg.timestamp.to_string(),
            msg.source_chunks,
            msg.confidence,
            msg.feedback.as_ref().map(|f| f.as_str().to_string()),
        ],
    )?;
    Ok(())
}

pub fn get_messages_by_conversation(
    conn: &Connection,
    conversation_id: &Uuid,
) -> Result<Vec<Message>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, role, content, timestamp, source_chunks, confidence, feedback
         FROM messages WHERE conversation_id = ?1 ORDER BY timestamp ASC",
    )?;

    let rows = stmt.query_map(params![conversation_id.to_string()], |row| {
        Ok(MessageRow {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            role: row.get(2)?,
            content: row.get(3)?,
            timestamp: row.get(4)?,
            source_chunks: row.get(5)?,
            confidence: row.get(6)?,
            feedback: row.get(7)?,
        })
    })?;

    let mut messages = Vec::new();
    for row in rows {
        messages.push(message_from_row(row?)?);
    }
    Ok(messages)
}

struct MessageRow {
    id: String,
    conversation_id: String,
    role: String,
    content: String,
    timestamp: String,
    source_chunks: Option<String>,
    confidence: Option<f32>,
    feedback: Option<String>,
}

/// Get concatenated lowercase text of recent user messages for topic detection.
/// Used by the suggestion scorer to suppress already-discussed topics.
pub fn get_recent_user_messages(conn: &Connection, hours: i64) -> Result<String, DatabaseError> {
    let modifier = format!("-{hours} hours");
    let mut stmt = conn.prepare(
        "SELECT content FROM messages
         WHERE role = 'patient' AND timestamp > datetime('now', ?1)
         ORDER BY timestamp DESC LIMIT 100",
    )?;

    let rows = stmt.query_map(params![modifier], |row| row.get::<_, String>(0))?;

    let mut text = String::new();
    for row in rows {
        let content = row?;
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&content.to_lowercase());
    }
    Ok(text)
}

fn message_from_row(row: MessageRow) -> Result<Message, DatabaseError> {
    Ok(Message {
        id: Uuid::parse_str(&row.id)
            .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
        conversation_id: Uuid::parse_str(&row.conversation_id)
            .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
        role: MessageRole::from_str(&row.role)?,
        content: row.content,
        timestamp: NaiveDateTime::parse_from_str(&row.timestamp, "%Y-%m-%d %H:%M:%S")
            .unwrap_or_default(),
        source_chunks: row.source_chunks,
        confidence: row.confidence,
        feedback: row
            .feedback
            .as_deref()
            .map(MessageFeedback::from_str)
            .transpose()?,
    })
}
