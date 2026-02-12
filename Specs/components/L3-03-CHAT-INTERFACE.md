# L3-03 — Chat Interface

<!--
=============================================================================
COMPONENT SPEC — The conversational interface. Where Marie asks questions.
Engineer review: E-UX (UI/UX, lead), E-RS (Rust), E-ML (AI/ML), E-SC (Security), E-QA (QA)
This is the core value proposition of Coheara: patients ask questions about
their medical documents and get grounded, sourced, streaming answers.
Every word, every animation, every citation must build trust.
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=20 limit=35` |
| [2] Dependencies | `offset=55 limit=20` |
| [3] Interfaces | `offset=75 limit=110` |
| [4] Message Display Rules | `offset=185 limit=50` |
| [5] Streaming Implementation | `offset=235 limit=65` |
| [6] Source Citations | `offset=300 limit=55` |
| [7] Confidence Display | `offset=355 limit=40` |
| [8] Feedback Mechanism | `offset=395 limit=45` |
| [9] Conversation Management | `offset=440 limit=65` |
| [10] Tauri Commands (IPC) | `offset=505 limit=80` |
| [11] Svelte Components | `offset=585 limit=310` |
| [12] Frontend API | `offset=895 limit=50` |
| [13] Error Handling | `offset=945 limit=30` |
| [14] Security | `offset=975 limit=25` |
| [15] Testing | `offset=1000 limit=65` |
| [16] Performance | `offset=1065 limit=20` |
| [17] Open Questions | `offset=1085 limit=15` |

---

## [1] Identity

**What:** The chat interface -- the conversational screen where patients ask questions about their medical documents and receive streaming, citation-grounded responses from MedGemma via the RAG pipeline. Includes: message list (patient right-aligned, Coheara left-aligned), streaming token-by-token display with subtle cursor, source citation chips below each response (tappable to view source document), per-response confidence indicator, "Was this helpful?" feedback widget, conversation list (sidebar/drawer), new conversation creation, conversation persistence in SQLite, patient context preamble injection, and empty state with encouraging prompt suggestions.

**After this session:**
- Patient navigates to chat from home quick action, tab bar, or appointment prep
- Empty state shows 4-6 prompt suggestions for first-time users or new conversations
- Patient types a question and presses send
- Streaming response appears token by token with a blinking cursor
- When streaming completes: citation chips appear below the response, confidence indicator shown, feedback widget fades in
- Patient taps citation chip to view the source document excerpt in a slide-up panel
- Patient taps thumbs up/down to rate the response (stored in messages table)
- Patient can start a new conversation or switch between past conversations
- Conversations persisted in SQLite: `conversations` table (id, started_at, title, profile_id) and `messages` table (id, conversation_id, role, content, timestamp, citations_json, confidence, feedback)
- Conversation title auto-generated from the first patient message (first 50 chars)
- Streaming via Tauri events: backend emits `chat-stream` events, frontend listens
- Patient context preamble injected on every query (active medications, known allergies)

**Estimated complexity:** High
**Source:** Tech Spec v1.1 Section 7 (Conversation Engine), L2-01 RAG Pipeline

---

## [2] Dependencies

**Incoming:**
- L2-01 (RAG pipeline -- `RagPipeline` trait, `query_streaming()`, `StreamChunk`, `Citation`, `ConversationManager`)
- L2-02 (safety filter -- post-generation filtering, boundary check enforcement)
- L0-03 (encryption -- `ProfileSession` for decrypting stored messages)
- L0-01 (project scaffold -- Tauri state management, event system)

**Outgoing:**
- L3-02 (home screen -- navigates here via quick action "Ask" or tab bar "Chat")
- L4-02 (appointment prep -- uses conversation context for appointment preparation)
- L3-04 (review screen -- citation chip tap can navigate to document detail)

**No new Cargo.toml dependencies.** Uses existing Tauri event system, `serde`, `uuid`, and repository traits.

---

## [3] Interfaces

### Backend Types

```rust
// src-tauri/src/models/conversation.rs

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A conversation session between patient and Coheara
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: Uuid,
    pub profile_id: Uuid,
    pub title: String,              // Auto-generated from first message
    pub started_at: NaiveDateTime,
    pub last_message_at: NaiveDateTime,
    pub message_count: u32,
}

/// A single message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: NaiveDateTime,
    pub citations_json: Option<String>,   // JSON-serialized Vec<Citation>
    pub confidence: Option<f32>,          // 0.0-1.0 for Coheara responses
    pub feedback: Option<MessageFeedback>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageRole {
    Patient,
    Coheara,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageFeedback {
    Helpful,
    NotHelpful,
}

/// A conversation summary for the conversation list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: Uuid,
    pub title: String,
    pub last_message_at: NaiveDateTime,
    pub message_count: u32,
    pub last_message_preview: String,   // First 80 chars of last message
}

/// Payload emitted via Tauri event during streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatStreamEvent {
    pub conversation_id: String,
    pub chunk: StreamChunkPayload,
}

/// A single streaming chunk sent to the frontend
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

/// Citation as displayed in the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationView {
    pub document_id: String,
    pub document_title: String,
    pub document_date: Option<String>,
    pub professional_name: Option<String>,
    pub chunk_text: String,
    pub relevance_score: f32,
}

/// Prompt suggestions for empty state / new conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptSuggestion {
    pub text: String,
    pub category: String,      // "medications", "labs", "general", "appointments"
}
```

### Frontend Types

```typescript
// src/lib/types/chat.ts

export interface Conversation {
  id: string;
  profile_id: string;
  title: string;
  started_at: string;
  last_message_at: string;
  message_count: number;
}

export interface Message {
  id: string;
  conversation_id: string;
  role: 'Patient' | 'Coheara';
  content: string;
  timestamp: string;
  citations: CitationView[] | null;
  confidence: number | null;
  feedback: 'Helpful' | 'NotHelpful' | null;
}

export interface ConversationSummary {
  id: string;
  title: string;
  last_message_at: string;
  message_count: number;
  last_message_preview: string;
}

export interface CitationView {
  document_id: string;
  document_title: string;
  document_date: string | null;
  professional_name: string | null;
  chunk_text: string;
  relevance_score: number;
}

export type StreamChunkPayload =
  | { type: 'Token'; text: string }
  | { type: 'Citation'; citation: CitationView }
  | { type: 'Done'; full_text: string; confidence: number; boundary_check: string }
  | { type: 'Error'; message: string };

export interface ChatStreamEvent {
  conversation_id: string;
  chunk: StreamChunkPayload;
}

export interface PromptSuggestion {
  text: string;
  category: string;
}
```

---

## [4] Message Display Rules

**E-UX lead:** The chat is a vertical scroll with messages. Calm, warm, trustworthy. No clinical coldness. No alarm wording. Marie is having a conversation with a patient assistant, not interrogating a database.

### Layout

```
+--------------------------------------------+
|  <- Back         Conversation Title      [] |  (header: back, title, menu)
+--------------------------------------------+
|                                             |
|  (Coheara avatar) Hello Marie, what would   |  (left-aligned, Coheara)
|  you like to know about your documents?     |
|                                             |
|         What dose of metformin am I on?  O  |  (right-aligned, Patient)
|                                             |
|  (Coheara avatar) Based on your             |  (left-aligned, Coheara)
|  prescription from Dr. Chen dated           |
|  January 15, your current dose of           |
|  metformin is 500mg taken twice daily,      |
|  once with breakfast and once with dinner.  |
|                                             |
|  [Dr. Chen - Jan 15] [Prescription]         |  (citation chips)
|                                             |
|  Confidence: Good                           |  (subtle indicator)
|  Was this helpful?  [thumbs up] [thumbs dn] |  (feedback widget)
|                                             |
+--------------------------------------------+
|  Ask about your documents...        [Send]  |  (input bar)
+--------------------------------------------+
```

### Patient Messages (Right-Aligned)

- Background: `bg-[var(--color-primary)]` with white text
- Rounded corners: `rounded-2xl rounded-br-md` (bubble with flat bottom-right)
- Max width: 80% of container
- Font: base size, regular weight
- Timestamp shown below message in stone-400, text-xs
- No avatar (patient's own messages need no attribution)

### Coheara Messages (Left-Aligned)

- Background: `bg-white` with stone-800 text
- Border: `border border-stone-100`
- Rounded corners: `rounded-2xl rounded-bl-md` (bubble with flat bottom-left)
- Max width: 85% of container
- Small Coheara avatar (circle with "C" or logo) to the left of the bubble
- Font: base size, regular weight
- Line height: relaxed (1.625) for readability of medical content
- Citation chips appear below the message bubble (after streaming completes)
- Confidence indicator appears below citations
- Feedback widget appears below confidence

### Streaming State

- While streaming: message bubble grows as tokens arrive
- Blinking cursor (subtle `|` character with `animate-pulse` at 1Hz)
- No citation chips until streaming completes
- No confidence or feedback until streaming completes
- "Thinking..." indicator shown during retrieval phase (before first token)

### Empty State

When the conversation has no messages (new conversation or first time):
- Centered Coheara greeting: "Hello {name}! I can help you understand your medical documents."
- Subtitle: "Ask me anything about your prescriptions, lab results, or health records."
- 4-6 prompt suggestion chips arranged in a 2-column grid below the greeting
- Tapping a suggestion fills the input and auto-sends

### Prompt Suggestions

```rust
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
```

---

## [5] Streaming Implementation

### Backend: Tauri Event Emission

The RAG pipeline (L2-01) calls `query_streaming()` which produces `StreamChunk` values via a callback. The Tauri command wraps this and emits each chunk as a `chat-stream` event.

```rust
// src-tauri/src/commands/chat.rs

use tauri::{AppHandle, Emitter, State};

/// Send a message and stream the response via Tauri events
#[tauri::command]
pub async fn send_chat_message(
    conversation_id: String,
    text: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conv_id = Uuid::parse_str(&conversation_id)
        .map_err(|e| format!("Invalid conversation ID: {e}"))?;

    // 1. Save patient message
    state.conversation_manager.add_patient_message(&conv_id, &text)
        .map_err(|e| e.to_string())?;

    // 2. Build query
    let query = PatientQuery {
        text: text.clone(),
        conversation_id: conv_id,
        query_type: None,  // Auto-classify
    };

    // 3. Stream response via Tauri events
    let app_handle = app.clone();
    let conv_id_str = conversation_id.clone();

    let response = state.rag_pipeline.query_streaming(
        &query,
        session,
        Box::new(move |chunk| {
            let payload = match chunk_to_payload(&chunk) {
                Some(p) => p,
                None => return,
            };
            let event = ChatStreamEvent {
                conversation_id: conv_id_str.clone(),
                chunk: payload,
            };
            let _ = app_handle.emit("chat-stream", &event);
        }),
    ).map_err(|e| e.to_string())?;

    // 4. Run safety filter (L2-02)
    let filtered = state.safety_filter.filter_response(&response)
        .map_err(|e| e.to_string())?;

    // 5. Save Coheara response
    state.conversation_manager.add_response(
        &conv_id,
        &filtered.text,
        &filtered.citations,
        filtered.confidence,
    ).map_err(|e| e.to_string())?;

    // 6. Emit final Done event
    let _ = app.emit("chat-stream", &ChatStreamEvent {
        conversation_id,
        chunk: StreamChunkPayload::Done {
            full_text: filtered.text,
            confidence: filtered.confidence,
            boundary_check: format!("{:?}", filtered.boundary_check),
        },
    });

    state.update_activity();
    Ok(())
}

/// Convert internal StreamChunk to frontend payload
fn chunk_to_payload(chunk: &StreamChunk) -> Option<StreamChunkPayload> {
    if chunk.is_final {
        // Final chunk handled separately after safety filter
        return None;
    }
    Some(StreamChunkPayload::Token {
        text: chunk.text.clone(),
    })
}
```

### Frontend: Event Listener

```typescript
// Listening pattern inside ChatScreen.svelte
import { listen } from '@tauri-apps/api/event';
import type { ChatStreamEvent, StreamChunkPayload } from '$lib/types/chat';

// Set up listener before sending the message
const unlisten = await listen<ChatStreamEvent>('chat-stream', (event) => {
  const { conversation_id, chunk } = event.payload;

  // Only process events for this conversation
  if (conversation_id !== currentConversationId) return;

  switch (chunk.type) {
    case 'Token':
      streamingText += chunk.text;
      break;
    case 'Citation':
      pendingCitations.push(chunk.citation);
      break;
    case 'Done':
      streamingText = chunk.full_text;
      responseConfidence = chunk.confidence;
      isStreaming = false;
      break;
    case 'Error':
      streamError = chunk.message;
      isStreaming = false;
      break;
  }
});
```

### Token-by-Token Display

The streaming text is rendered incrementally. As each `Token` event arrives, it appends to `streamingText`. The Svelte reactivity system re-renders the message bubble on each update.

```
Timeline:
  T+0ms    Patient sends message
  T+50ms   "Thinking..." indicator shown (retrieval in progress)
  T+500ms  First Token event: "Based" → bubble appears with "Based|"
  T+520ms  Token: " on" → "Based on|"
  T+540ms  Token: " your" → "Based on your|"
  ...      (tokens arrive at ~20ms intervals for MedGemma 4B)
  T+8000ms Done event → cursor removed, citations shown, confidence shown
```

The blinking cursor is a `<span>` with `animate-pulse` appended after the last token. It is removed when `isStreaming` becomes false.

---

## [6] Source Citations

### Citation Display

Citations appear as horizontal-scrolling chips below the completed Coheara response message. They are only shown after streaming completes (never during streaming).

```
+-------------------------------------------+
|  Based on your prescription from Dr. Chen  |
|  dated January 15, your current dose of    |
|  metformin is 500mg taken twice daily.     |
+-------------------------------------------+
  [Dr. Chen - Jan 15]  [Lab Central - Jan 10]
```

### Citation Chip Anatomy

Each chip shows:
- Professional name (if available) or document type as fallback
- Document date (formatted as "Mon DD")
- Subtle left border color based on document type (prescription=blue, lab=green, referral=purple)

### Citation Chip Tap Behavior

Tapping a citation chip opens a slide-up panel (bottom sheet) that shows:
1. **Header:** Document type, professional name, date
2. **Excerpt:** The `chunk_text` from the citation (the passage that supports the claim)
3. **Relevance:** Visual relevance bar (maps `relevance_score` to width)
4. **Action button:** "View full document" navigates to L3-04 document detail

The panel covers approximately 60% of the screen height and can be dismissed by swiping down or tapping the backdrop.

### Citation Extraction from RAG Response

The RAG pipeline (L2-01) extracts citations via `extract_citations()` which finds `[Doc: uuid, Date: date]` markers in MedGemma's output and maps them to source chunks. The cleaned text (markers removed by `clean_citations_for_display()`) is what the patient sees. The extracted citations are sent as `CitationView` objects.

```rust
// Transform L2-01 Citation to frontend CitationView
impl From<Citation> for CitationView {
    fn from(c: Citation) -> Self {
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
```

---

## [7] Confidence Display

### Confidence Indicator

Each Coheara response has a confidence score (0.0 - 1.0) computed by the RAG pipeline based on:
- Retrieval relevance scores (how closely chunks matched the query)
- Number of supporting sources
- Whether structured data corroborated semantic results

### Display Rules

**E-UX lead:** The confidence indicator must be calm and informative, never alarming. Patients should not panic over a low-confidence answer.

| Confidence Range | Label | Color | Icon |
|-----------------|-------|-------|------|
| 0.8 - 1.0 | "Well supported" | `text-green-600` | Solid circle |
| 0.5 - 0.79 | "Partially supported" | `text-amber-600` | Half circle |
| 0.0 - 0.49 | "Limited information" | `text-stone-400` | Empty circle |

### Visual Design

- Shown below citation chips, left-aligned
- Small text (text-xs), subtle, not attention-grabbing
- Format: `{icon} {label}` (e.g., "&#9679; Well supported")
- Tooltip on hover/long-press: "This indicates how much of this answer is directly supported by your documents."

### No-Context Response

When the RAG pipeline finds no relevant context (`RagError::NoContext`), the response says:
> "I don't have enough information in your documents to answer this question. You might want to load more documents or ask your healthcare provider."

Confidence is displayed as "Limited information" with a helpful note.

---

## [8] Feedback Mechanism

### "Was this helpful?" Widget

Appears below the confidence indicator after streaming completes. Two buttons: thumbs up and thumbs down.

### Feedback Flow

1. Widget appears with both buttons in neutral state (stone-300 outline)
2. Patient taps thumbs up:
   - Button fills with green (`bg-green-100 text-green-600`)
   - Other button fades (`opacity-30`)
   - `feedback: 'Helpful'` saved to `messages` table
   - Brief "Thank you!" text appears and fades after 2 seconds
3. Patient taps thumbs down:
   - Button fills with stone (`bg-stone-100 text-stone-600`)
   - Other button fades (`opacity-30`)
   - `feedback: 'NotHelpful'` saved to `messages` table
   - Brief "Thank you for the feedback" text appears and fades after 2 seconds
4. Tapping again on the same button deselects (clears feedback)
5. Tapping the other button switches the selection

### Persistence

```rust
/// Update feedback for a message
#[tauri::command]
pub async fn set_message_feedback(
    message_id: String,
    feedback: Option<String>,  // "Helpful", "NotHelpful", or null to clear
    state: State<'_, AppState>,
) -> Result<(), String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let msg_id = Uuid::parse_str(&message_id)
        .map_err(|e| format!("Invalid message ID: {e}"))?;

    let feedback_enum = match feedback.as_deref() {
        Some("Helpful") => Some(MessageFeedback::Helpful),
        Some("NotHelpful") => Some(MessageFeedback::NotHelpful),
        _ => None,
    };

    let conn = session.db_connection()
        .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE messages SET feedback = ?1 WHERE id = ?2",
        rusqlite::params![
            feedback_enum.map(|f| match f {
                MessageFeedback::Helpful => "helpful",
                MessageFeedback::NotHelpful => "not_helpful",
            }),
            msg_id,
        ],
    ).map_err(|e| format!("Failed to save feedback: {e}"))?;

    state.update_activity();
    Ok(())
}
```

### Feedback Analytics

Feedback data stays local. It is used to:
- Improve prompt suggestions (surface categories with more helpful responses)
- Track user satisfaction over time (visible in future settings/stats screen)
- No data leaves the device

---

## [9] Conversation Management

### Conversation Lifecycle

```
New conversation → First message sent → Title generated → Messages exchanged → Conversation persisted
```

### Title Generation

The conversation title is automatically derived from the first patient message:
- Take the first 50 characters of the first patient message
- If the message is longer than 50 chars, append "..."
- Title is editable later (future enhancement, not in Phase 1)

```rust
/// Generate conversation title from first message
fn generate_title(first_message: &str) -> String {
    let trimmed = first_message.trim();
    if trimmed.len() <= 50 {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..trimmed.char_indices()
            .take_while(|(i, _)| *i < 50)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(50)])
    }
}
```

### Conversation List

A drawer/sidebar showing past conversations, ordered by `last_message_at DESC`.

| Field | Display |
|-------|---------|
| Title | Truncated to 1 line, font-medium |
| Preview | Last message preview, text-sm, text-stone-500, 1 line |
| Time | Relative time ("2h ago", "Yesterday", "Jan 15"), text-xs, text-stone-400 |
| Unread indicator | None for Phase 1 (all conversations are read) |

### Conversation List Access

- **Mobile/narrow:** Hamburger icon in chat header opens a slide-in drawer from left
- **Desktop/wide:** Persistent left sidebar (280px width)
- "New conversation" button at the top of the list

### Conversation Switching

When the patient selects a different conversation:
1. Current streaming (if any) is cancelled (stop listening to events)
2. Message list replaced with selected conversation's messages
3. Messages loaded from SQLite via `get_conversation_messages`
4. Scroll to bottom of loaded messages
5. Input cleared

### New Conversation

When the patient starts a new conversation:
1. `start_conversation` Tauri command called
2. Returns new conversation ID
3. Message list cleared
4. Empty state / prompt suggestions shown
5. New conversation appears in conversation list

### Conversation Deletion

Swipe left on a conversation in the list shows a "Delete" button. Confirmation required:
- "Delete this conversation? This cannot be undone."
- Deletes conversation and all its messages from SQLite

### Database Schema

```sql
-- Conversations table
CREATE TABLE IF NOT EXISTS conversations (
    id TEXT PRIMARY KEY,               -- UUID
    profile_id TEXT NOT NULL,          -- FK to profile
    title TEXT NOT NULL,
    started_at TEXT NOT NULL,          -- ISO 8601
    last_message_at TEXT NOT NULL,     -- ISO 8601
    message_count INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (profile_id) REFERENCES profiles(id)
);

CREATE INDEX idx_conversations_profile ON conversations(profile_id);
CREATE INDEX idx_conversations_last_message ON conversations(last_message_at DESC);

-- Messages table
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,               -- UUID
    conversation_id TEXT NOT NULL,     -- FK to conversations
    role TEXT NOT NULL,                -- 'patient' or 'coheara'
    content TEXT NOT NULL,             -- Encrypted message content
    timestamp TEXT NOT NULL,           -- ISO 8601
    citations_json TEXT,              -- JSON array of CitationView objects, encrypted
    confidence REAL,                   -- 0.0-1.0 for Coheara responses
    feedback TEXT,                     -- 'helpful', 'not_helpful', or NULL
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);

CREATE INDEX idx_messages_conversation ON messages(conversation_id);
CREATE INDEX idx_messages_timestamp ON messages(timestamp);
```

---

## [10] Tauri Commands (IPC)

```rust
// src-tauri/src/commands/chat.rs

use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

/// Start a new conversation
#[tauri::command]
pub async fn start_conversation(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conn = session.db_connection()
        .map_err(|e| e.to_string())?;

    let id = Uuid::new_v4();
    let now = chrono::Local::now().naive_local();

    conn.execute(
        "INSERT INTO conversations (id, profile_id, title, started_at, last_message_at, message_count)
         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
        rusqlite::params![
            id.to_string(),
            session.profile_id.to_string(),
            "New conversation",
            now.to_string(),
            now.to_string(),
        ],
    ).map_err(|e| format!("Failed to create conversation: {e}"))?;

    state.update_activity();
    Ok(id.to_string())
}

/// Send a message and stream the response (defined in Section [5])
/// See streaming implementation above for full `send_chat_message` command.

/// Get all messages for a conversation
#[tauri::command]
pub async fn get_conversation_messages(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Message>, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conv_id = Uuid::parse_str(&conversation_id)
        .map_err(|e| format!("Invalid ID: {e}"))?;

    let conn = session.db_connection()
        .map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, role, content, timestamp, citations_json, confidence, feedback
         FROM messages
         WHERE conversation_id = ?1
         ORDER BY timestamp ASC"
    ).map_err(|e| format!("Query error: {e}"))?;

    let messages = stmt.query_map(rusqlite::params![conv_id.to_string()], |row| {
        let role_str: String = row.get(2)?;
        let feedback_str: Option<String> = row.get(7)?;

        Ok(Message {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            role: match role_str.as_str() {
                "patient" => MessageRole::Patient,
                _ => MessageRole::Coheara,
            },
            content: row.get(3)?,
            timestamp: row.get(4)?,
            citations_json: row.get(5)?,
            confidence: row.get(6)?,
            feedback: feedback_str.and_then(|f| match f.as_str() {
                "helpful" => Some(MessageFeedback::Helpful),
                "not_helpful" => Some(MessageFeedback::NotHelpful),
                _ => None,
            }),
        })
    }).map_err(|e| format!("Query error: {e}"))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| format!("Row mapping error: {e}"))?;

    state.update_activity();
    Ok(messages)
}

/// List conversations for the current profile
#[tauri::command]
pub async fn list_conversations(
    state: State<'_, AppState>,
) -> Result<Vec<ConversationSummary>, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conn = session.db_connection()
        .map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT c.id, c.title, c.last_message_at, c.message_count,
                COALESCE(
                    (SELECT SUBSTR(m.content, 1, 80) FROM messages m
                     WHERE m.conversation_id = c.id
                     ORDER BY m.timestamp DESC LIMIT 1),
                    ''
                ) AS last_preview
         FROM conversations c
         WHERE c.profile_id = ?1
         ORDER BY c.last_message_at DESC"
    ).map_err(|e| format!("Query error: {e}"))?;

    let conversations = stmt.query_map(
        rusqlite::params![session.profile_id.to_string()],
        |row| {
            Ok(ConversationSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                last_message_at: row.get(2)?,
                message_count: row.get(3)?,
                last_message_preview: row.get(4)?,
            })
        },
    ).map_err(|e| format!("Query error: {e}"))?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| format!("Row mapping error: {e}"))?;

    state.update_activity();
    Ok(conversations)
}

/// Delete a conversation and all its messages
#[tauri::command]
pub async fn delete_conversation(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conv_id = Uuid::parse_str(&conversation_id)
        .map_err(|e| format!("Invalid ID: {e}"))?;

    let conn = session.db_connection()
        .map_err(|e| e.to_string())?;

    // CASCADE deletes messages
    conn.execute(
        "DELETE FROM conversations WHERE id = ?1 AND profile_id = ?2",
        rusqlite::params![conv_id.to_string(), session.profile_id.to_string()],
    ).map_err(|e| format!("Delete failed: {e}"))?;

    state.update_activity();
    Ok(())
}

/// Save feedback for a message (defined in Section [8])
/// See feedback mechanism above for full `set_message_feedback` command.

/// Get prompt suggestions based on profile data
#[tauri::command]
pub async fn get_prompt_suggestions(
    state: State<'_, AppState>,
) -> Result<Vec<PromptSuggestion>, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conn = session.db_connection()
        .map_err(|e| e.to_string())?;

    // Check what data exists to tailor suggestions
    let has_meds: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM medications", [], |row| row.get(0)
    ).unwrap_or(false);

    let has_labs: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM lab_results", [], |row| row.get(0)
    ).unwrap_or(false);

    let mut suggestions = default_prompt_suggestions();

    // Add contextual suggestions based on available data
    if has_meds {
        suggestions.push(PromptSuggestion {
            text: "Do any of my medications have common side effects?".into(),
            category: "medications".into(),
        });
    }
    if has_labs {
        suggestions.push(PromptSuggestion {
            text: "Are any of my lab values outside the normal range?".into(),
            category: "labs".into(),
        });
    }

    // Return at most 6 suggestions
    suggestions.truncate(6);

    state.update_activity();
    Ok(suggestions)
}
```

---

## [11] Svelte Components

### ChatScreen (Main Container)

```svelte
<!-- src/lib/components/chat/ChatScreen.svelte -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import {
    startConversation,
    sendChatMessage,
    getConversationMessages,
    listConversations,
    getPromptSuggestions,
  } from '$lib/api/chat';
  import type {
    Message,
    ConversationSummary,
    ChatStreamEvent,
    CitationView,
    PromptSuggestion,
  } from '$lib/types/chat';
  import MessageBubble from './MessageBubble.svelte';
  import StreamingIndicator from './StreamingIndicator.svelte';
  import ConversationList from './ConversationList.svelte';
  import ChatEmptyState from './ChatEmptyState.svelte';

  interface Props {
    profileName: string;
    onNavigate: (screen: string, params?: Record<string, string>) => void;
    initialConversationId?: string;
  }
  let { profileName, onNavigate, initialConversationId }: Props = $props();

  // Conversation state
  let currentConversationId: string | null = $state(initialConversationId ?? null);
  let messages: Message[] = $state([]);
  let conversations: ConversationSummary[] = $state([]);
  let suggestions: PromptSuggestion[] = $state([]);

  // Input state
  let inputText = $state('');
  let isSending = $state(false);

  // Streaming state
  let isStreaming = $state(false);
  let streamingText = $state('');
  let pendingCitations: CitationView[] = $state([]);
  let responseConfidence: number | null = $state(null);
  let streamError: string | null = $state(null);

  // UI state
  let showConversationList = $state(false);
  let messageContainer: HTMLElement | undefined = $state(undefined);

  // Derived
  let hasMessages = $derived(messages.length > 0);
  let conversationTitle = $derived(
    conversations.find(c => c.id === currentConversationId)?.title ?? 'New conversation'
  );
  let canSend = $derived(inputText.trim().length > 0 && !isSending && !isStreaming);

  // Scroll to bottom when messages change
  function scrollToBottom() {
    if (messageContainer) {
      requestAnimationFrame(() => {
        messageContainer!.scrollTop = messageContainer!.scrollHeight;
      });
    }
  }

  // Load conversation messages
  async function loadMessages(convId: string) {
    try {
      const loaded = await getConversationMessages(convId);
      messages = loaded.map(m => ({
        ...m,
        citations: m.citations_json ? JSON.parse(m.citations_json) : null,
      }));
      scrollToBottom();
    } catch (e) {
      console.error('Failed to load messages:', e);
    }
  }

  // Load conversation list
  async function loadConversations() {
    try {
      conversations = await listConversations();
    } catch (e) {
      console.error('Failed to load conversations:', e);
    }
  }

  // Start a new conversation
  async function handleNewConversation() {
    try {
      const id = await startConversation();
      currentConversationId = id;
      messages = [];
      streamingText = '';
      pendingCitations = [];
      responseConfidence = null;
      streamError = null;
      showConversationList = false;
      await loadConversations();
    } catch (e) {
      console.error('Failed to start conversation:', e);
    }
  }

  // Switch to an existing conversation
  async function handleSelectConversation(convId: string) {
    if (isStreaming) return; // Don't switch during streaming
    currentConversationId = convId;
    streamingText = '';
    pendingCitations = [];
    responseConfidence = null;
    streamError = null;
    showConversationList = false;
    await loadMessages(convId);
  }

  // Send a message
  async function handleSend() {
    if (!canSend) return;

    const text = inputText.trim();
    inputText = '';

    // Create conversation if needed
    if (!currentConversationId) {
      try {
        const id = await startConversation();
        currentConversationId = id;
      } catch (e) {
        streamError = 'Could not start a conversation. Please try again.';
        return;
      }
    }

    // Add patient message to local state immediately
    const patientMessage: Message = {
      id: crypto.randomUUID(),
      conversation_id: currentConversationId!,
      role: 'Patient',
      content: text,
      timestamp: new Date().toISOString(),
      citations: null,
      confidence: null,
      feedback: null,
    };
    messages = [...messages, patientMessage];
    scrollToBottom();

    // Start streaming
    isSending = true;
    isStreaming = true;
    streamingText = '';
    pendingCitations = [];
    responseConfidence = null;
    streamError = null;

    // Listen for stream events
    const unlisten = await listen<ChatStreamEvent>('chat-stream', (event) => {
      const { conversation_id, chunk } = event.payload;
      if (conversation_id !== currentConversationId) return;

      switch (chunk.type) {
        case 'Token':
          streamingText += chunk.text;
          scrollToBottom();
          break;
        case 'Citation':
          pendingCitations = [...pendingCitations, chunk.citation];
          break;
        case 'Done':
          streamingText = chunk.full_text;
          responseConfidence = chunk.confidence;
          isStreaming = false;

          // Add completed Coheara message to local state
          const cohearaMessage: Message = {
            id: crypto.randomUUID(),
            conversation_id: currentConversationId!,
            role: 'Coheara',
            content: chunk.full_text,
            timestamp: new Date().toISOString(),
            citations: pendingCitations.length > 0 ? [...pendingCitations] : null,
            confidence: chunk.confidence,
            feedback: null,
          };
          messages = [...messages, cohearaMessage];
          streamingText = '';
          pendingCitations = [];
          scrollToBottom();
          break;
        case 'Error':
          streamError = chunk.message;
          isStreaming = false;
          break;
      }
    });

    try {
      await sendChatMessage(currentConversationId!, text);
    } catch (e) {
      streamError = e instanceof Error ? e.message : String(e);
      isStreaming = false;
    } finally {
      unlisten();
      isSending = false;
      await loadConversations(); // Refresh list for updated title/preview
    }
  }

  // Handle suggestion tap
  function handleSuggestionTap(suggestion: PromptSuggestion) {
    inputText = suggestion.text;
    handleSend();
  }

  // Handle keyboard submit
  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }

  onMount(async () => {
    await loadConversations();
    suggestions = await getPromptSuggestions();

    if (currentConversationId) {
      await loadMessages(currentConversationId);
    }
  });
</script>

<div class="flex flex-col h-full bg-stone-50">
  <!-- Header -->
  <header class="flex items-center gap-3 px-4 py-3 bg-white border-b border-stone-200">
    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center text-stone-400
             hover:text-stone-600"
      onclick={() => showConversationList = !showConversationList}
      aria-label="Toggle conversation list"
    >
      <span class="text-xl">&equiv;</span>
    </button>

    <h1 class="flex-1 text-base font-medium text-stone-800 truncate">
      {conversationTitle}
    </h1>

    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center text-stone-400
             hover:text-[var(--color-primary)]"
      onclick={handleNewConversation}
      aria-label="Start new conversation"
    >
      <span class="text-xl">+</span>
    </button>
  </header>

  <!-- Conversation list drawer -->
  {#if showConversationList}
    <div class="absolute inset-0 z-40 flex">
      <!-- Backdrop -->
      <button
        class="absolute inset-0 bg-black/20"
        onclick={() => showConversationList = false}
        aria-label="Close conversation list"
      ></button>
      <!-- Drawer -->
      <div class="relative z-50 w-[280px] bg-white h-full shadow-xl overflow-y-auto">
        <ConversationList
          {conversations}
          activeConversationId={currentConversationId}
          onSelect={handleSelectConversation}
          onNewConversation={handleNewConversation}
          onDelete={async (id) => {
            const { deleteConversation } = await import('$lib/api/chat');
            await deleteConversation(id);
            if (currentConversationId === id) {
              currentConversationId = null;
              messages = [];
            }
            await loadConversations();
          }}
        />
      </div>
    </div>
  {/if}

  <!-- Messages area -->
  <div
    class="flex-1 overflow-y-auto px-4 py-4"
    bind:this={messageContainer}
    role="log"
    aria-label="Conversation messages"
    aria-live="polite"
  >
    {#if !hasMessages && !isStreaming}
      <ChatEmptyState
        {profileName}
        {suggestions}
        onSuggestionTap={handleSuggestionTap}
      />
    {:else}
      <div class="flex flex-col gap-4 max-w-2xl mx-auto">
        {#each messages as message (message.id)}
          <MessageBubble
            {message}
            {onNavigate}
          />
        {/each}

        <!-- Streaming message (not yet in messages array) -->
        {#if isStreaming && streamingText}
          <div class="flex items-start gap-2">
            <!-- Coheara avatar -->
            <div class="w-8 h-8 rounded-full bg-[var(--color-primary)] flex items-center
                        justify-center text-white text-sm font-bold flex-shrink-0 mt-1">
              C
            </div>
            <div class="max-w-[85%] bg-white border border-stone-100 rounded-2xl rounded-bl-md
                        px-4 py-3 shadow-sm">
              <p class="text-stone-800 text-base leading-relaxed whitespace-pre-wrap">
                {streamingText}<span class="animate-pulse text-[var(--color-primary)]">|</span>
              </p>
            </div>
          </div>
        {:else if isStreaming && !streamingText}
          <StreamingIndicator />
        {/if}

        <!-- Stream error -->
        {#if streamError}
          <div class="flex items-start gap-2">
            <div class="w-8 h-8 rounded-full bg-stone-200 flex items-center justify-center
                        text-stone-500 text-sm font-bold flex-shrink-0 mt-1">
              C
            </div>
            <div class="max-w-[85%] bg-amber-50 border border-amber-200 rounded-2xl rounded-bl-md
                        px-4 py-3">
              <p class="text-amber-800 text-sm">{streamError}</p>
              <button
                class="text-amber-700 text-sm font-medium mt-2 underline min-h-[44px]"
                onclick={() => { streamError = null; }}
              >
                Dismiss
              </button>
            </div>
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <!-- Input bar -->
  <div class="bg-white border-t border-stone-200 px-4 py-3">
    <div class="flex items-end gap-2 max-w-2xl mx-auto">
      <textarea
        class="flex-1 px-4 py-3 rounded-xl border border-stone-200 text-base
               resize-none min-h-[44px] max-h-[120px]
               focus:border-[var(--color-primary)] focus:outline-none
               placeholder:text-stone-400"
        placeholder="Ask about your documents..."
        bind:value={inputText}
        onkeydown={handleKeydown}
        rows={1}
        disabled={isStreaming}
        aria-label="Type your question"
      ></textarea>
      <button
        class="min-h-[44px] min-w-[44px] px-4 py-3 rounded-xl font-medium text-base
               transition-colors
               {canSend
                 ? 'bg-[var(--color-primary)] text-white hover:brightness-110'
                 : 'bg-stone-100 text-stone-400 cursor-not-allowed'}"
        onclick={handleSend}
        disabled={!canSend}
        aria-label="Send message"
      >
        Send
      </button>
    </div>
  </div>
</div>
```

### MessageBubble

```svelte
<!-- src/lib/components/chat/MessageBubble.svelte -->
<script lang="ts">
  import type { Message } from '$lib/types/chat';
  import CitationChip from './CitationChip.svelte';
  import ConfidenceIndicator from './ConfidenceIndicator.svelte';
  import FeedbackWidget from './FeedbackWidget.svelte';

  interface Props {
    message: Message;
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { message, onNavigate }: Props = $props();

  let isPatient = $derived(message.role === 'Patient');

  function formatTime(timestamp: string): string {
    const date = new Date(timestamp);
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }
</script>

{#if isPatient}
  <!-- Patient message: right-aligned -->
  <div class="flex justify-end">
    <div class="max-w-[80%] flex flex-col items-end">
      <div class="bg-[var(--color-primary)] text-white rounded-2xl rounded-br-md px-4 py-3">
        <p class="text-base leading-relaxed whitespace-pre-wrap">{message.content}</p>
      </div>
      <span class="text-xs text-stone-400 mt-1 mr-1">{formatTime(message.timestamp)}</span>
    </div>
  </div>
{:else}
  <!-- Coheara message: left-aligned -->
  <div class="flex items-start gap-2">
    <!-- Avatar -->
    <div class="w-8 h-8 rounded-full bg-[var(--color-primary)] flex items-center justify-center
                text-white text-sm font-bold flex-shrink-0 mt-1">
      C
    </div>
    <div class="max-w-[85%] flex flex-col items-start">
      <div class="bg-white border border-stone-100 rounded-2xl rounded-bl-md px-4 py-3 shadow-sm">
        <p class="text-stone-800 text-base leading-relaxed whitespace-pre-wrap">{message.content}</p>
      </div>

      <!-- Citations -->
      {#if message.citations && message.citations.length > 0}
        <div class="flex flex-wrap gap-2 mt-2 ml-1">
          {#each message.citations as citation}
            <CitationChip {citation} {onNavigate} />
          {/each}
        </div>
      {/if}

      <!-- Confidence -->
      {#if message.confidence !== null}
        <div class="mt-2 ml-1">
          <ConfidenceIndicator confidence={message.confidence} />
        </div>
      {/if}

      <!-- Feedback -->
      <div class="mt-2 ml-1">
        <FeedbackWidget
          messageId={message.id}
          currentFeedback={message.feedback}
        />
      </div>

      <span class="text-xs text-stone-400 mt-1 ml-1">{formatTime(message.timestamp)}</span>
    </div>
  </div>
{/if}
```

### CitationChip

```svelte
<!-- src/lib/components/chat/CitationChip.svelte -->
<script lang="ts">
  import type { CitationView } from '$lib/types/chat';

  interface Props {
    citation: CitationView;
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { citation, onNavigate }: Props = $props();

  let showPanel = $state(false);

  let displayLabel = $derived(
    citation.professional_name
      ?? citation.document_title
      ?? 'Source document'
  );

  let displayDate = $derived(() => {
    if (!citation.document_date) return '';
    const date = new Date(citation.document_date);
    return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
  });

  // Relevance bar width (0-100%)
  let relevanceWidth = $derived(Math.round(citation.relevance_score * 100));
</script>

<!-- Citation chip button -->
<button
  class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full
         bg-stone-100 hover:bg-stone-200 border border-stone-200
         text-xs text-stone-700 transition-colors
         min-h-[32px]"
  onclick={() => showPanel = true}
  aria-label="View source: {displayLabel}"
>
  <span class="w-1.5 h-1.5 rounded-full bg-blue-400 flex-shrink-0"></span>
  <span class="truncate max-w-[140px]">{displayLabel}</span>
  {#if displayDate()}
    <span class="text-stone-400">- {displayDate()}</span>
  {/if}
</button>

<!-- Slide-up panel (bottom sheet) -->
{#if showPanel}
  <div class="fixed inset-0 z-50 flex flex-col justify-end">
    <!-- Backdrop -->
    <button
      class="absolute inset-0 bg-black/30"
      onclick={() => showPanel = false}
      aria-label="Close citation panel"
    ></button>

    <!-- Panel -->
    <div class="relative bg-white rounded-t-2xl shadow-xl max-h-[60vh] overflow-y-auto
                animate-slide-up">
      <!-- Drag handle -->
      <div class="flex justify-center py-3">
        <div class="w-10 h-1 rounded-full bg-stone-300"></div>
      </div>

      <div class="px-6 pb-8">
        <!-- Header -->
        <div class="mb-4">
          <h3 class="text-lg font-medium text-stone-800">
            {citation.document_title}
          </h3>
          <div class="flex items-center gap-2 mt-1 text-sm text-stone-500">
            {#if citation.professional_name}
              <span>{citation.professional_name}</span>
            {/if}
            {#if citation.document_date}
              <span>- {citation.document_date}</span>
            {/if}
          </div>
        </div>

        <!-- Excerpt -->
        <div class="mb-4">
          <h4 class="text-xs font-medium text-stone-400 uppercase mb-2">Source excerpt</h4>
          <p class="text-sm text-stone-700 leading-relaxed bg-stone-50 rounded-lg p-3 border border-stone-100">
            {citation.chunk_text}
          </p>
        </div>

        <!-- Relevance -->
        <div class="mb-6">
          <h4 class="text-xs font-medium text-stone-400 uppercase mb-2">Relevance</h4>
          <div class="flex items-center gap-2">
            <div class="flex-1 h-2 bg-stone-100 rounded-full overflow-hidden">
              <div
                class="h-full bg-[var(--color-primary)] rounded-full transition-all"
                style="width: {relevanceWidth}%"
              ></div>
            </div>
            <span class="text-xs text-stone-500">{relevanceWidth}%</span>
          </div>
        </div>

        <!-- Action -->
        <button
          class="w-full px-6 py-3 bg-stone-100 text-stone-700 rounded-xl font-medium
                 hover:bg-stone-200 transition-colors min-h-[44px]"
          onclick={() => {
            showPanel = false;
            onNavigate('document-detail', { documentId: citation.document_id });
          }}
        >
          View full document
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  @keyframes slide-up {
    from { transform: translateY(100%); }
    to { transform: translateY(0); }
  }
  .animate-slide-up {
    animation: slide-up 0.3s ease-out;
  }
</style>
```

### StreamingIndicator

```svelte
<!-- src/lib/components/chat/StreamingIndicator.svelte -->
<script lang="ts">
  // No props needed
</script>

<div class="flex items-start gap-2">
  <!-- Coheara avatar -->
  <div class="w-8 h-8 rounded-full bg-[var(--color-primary)] flex items-center justify-center
              text-white text-sm font-bold flex-shrink-0 mt-1">
    C
  </div>
  <div class="bg-white border border-stone-100 rounded-2xl rounded-bl-md px-4 py-3 shadow-sm">
    <div class="flex items-center gap-1.5">
      <span class="w-2 h-2 rounded-full bg-stone-300 animate-bounce" style="animation-delay: 0ms"></span>
      <span class="w-2 h-2 rounded-full bg-stone-300 animate-bounce" style="animation-delay: 150ms"></span>
      <span class="w-2 h-2 rounded-full bg-stone-300 animate-bounce" style="animation-delay: 300ms"></span>
    </div>
    <p class="text-xs text-stone-400 mt-1">Searching your documents...</p>
  </div>
</div>
```

### ConfidenceIndicator

```svelte
<!-- src/lib/components/chat/ConfidenceIndicator.svelte -->
<script lang="ts">
  interface Props {
    confidence: number;
  }
  let { confidence }: Props = $props();

  let display = $derived(() => {
    if (confidence >= 0.8) {
      return { label: 'Well supported', color: 'text-green-600', icon: '\u25CF' }; // solid circle
    } else if (confidence >= 0.5) {
      return { label: 'Partially supported', color: 'text-amber-600', icon: '\u25D0' }; // half circle
    } else {
      return { label: 'Limited information', color: 'text-stone-400', icon: '\u25CB' }; // empty circle
    }
  });
</script>

<div
  class="flex items-center gap-1.5 text-xs {display().color}"
  title="This indicates how much of this answer is directly supported by your documents."
  role="status"
  aria-label="Confidence: {display().label}"
>
  <span aria-hidden="true">{display().icon}</span>
  <span>{display().label}</span>
</div>
```

### FeedbackWidget

```svelte
<!-- src/lib/components/chat/FeedbackWidget.svelte -->
<script lang="ts">
  import { setMessageFeedback } from '$lib/api/chat';

  interface Props {
    messageId: string;
    currentFeedback: 'Helpful' | 'NotHelpful' | null;
  }
  let { messageId, currentFeedback }: Props = $props();

  let feedback: 'Helpful' | 'NotHelpful' | null = $state(currentFeedback);
  let showThankYou = $state(false);
  let saving = $state(false);

  async function handleFeedback(value: 'Helpful' | 'NotHelpful') {
    if (saving) return;
    saving = true;

    try {
      if (feedback === value) {
        // Deselect
        feedback = null;
        await setMessageFeedback(messageId, null);
      } else {
        // Select
        feedback = value;
        await setMessageFeedback(messageId, value);
        showThankYou = true;
        setTimeout(() => { showThankYou = false; }, 2000);
      }
    } catch (e) {
      console.error('Failed to save feedback:', e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="flex items-center gap-2">
  <span class="text-xs text-stone-400">Was this helpful?</span>

  <button
    class="min-h-[32px] min-w-[32px] flex items-center justify-center rounded-full
           transition-all
           {feedback === 'Helpful'
             ? 'bg-green-100 text-green-600'
             : feedback === 'NotHelpful'
               ? 'opacity-30 text-stone-400'
               : 'text-stone-400 hover:bg-stone-100'}"
    onclick={() => handleFeedback('Helpful')}
    aria-label="Helpful"
    aria-pressed={feedback === 'Helpful'}
    disabled={saving}
  >
    <span class="text-sm" aria-hidden="true">&#128077;</span>
  </button>

  <button
    class="min-h-[32px] min-w-[32px] flex items-center justify-center rounded-full
           transition-all
           {feedback === 'NotHelpful'
             ? 'bg-stone-100 text-stone-600'
             : feedback === 'Helpful'
               ? 'opacity-30 text-stone-400'
               : 'text-stone-400 hover:bg-stone-100'}"
    onclick={() => handleFeedback('NotHelpful')}
    aria-label="Not helpful"
    aria-pressed={feedback === 'NotHelpful'}
    disabled={saving}
  >
    <span class="text-sm" aria-hidden="true">&#128078;</span>
  </button>

  {#if showThankYou}
    <span class="text-xs text-stone-400 animate-fade-out">
      {feedback === 'Helpful' ? 'Thank you!' : 'Thank you for the feedback'}
    </span>
  {/if}
</div>

<style>
  @keyframes fade-out {
    0% { opacity: 1; }
    70% { opacity: 1; }
    100% { opacity: 0; }
  }
  .animate-fade-out {
    animation: fade-out 2s ease-out forwards;
  }
</style>
```

### ConversationList

```svelte
<!-- src/lib/components/chat/ConversationList.svelte -->
<script lang="ts">
  import type { ConversationSummary } from '$lib/types/chat';

  interface Props {
    conversations: ConversationSummary[];
    activeConversationId: string | null;
    onSelect: (id: string) => void;
    onNewConversation: () => void;
    onDelete: (id: string) => void;
  }
  let { conversations, activeConversationId, onSelect, onNewConversation, onDelete }: Props = $props();

  let confirmDeleteId: string | null = $state(null);

  function relativeTime(dateStr: string): string {
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    if (diffMins < 1) return 'Just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    const diffHours = Math.floor(diffMins / 60);
    if (diffHours < 24) return `${diffHours}h ago`;
    const diffDays = Math.floor(diffHours / 24);
    if (diffDays < 7) return `${diffDays}d ago`;
    return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
  }
</script>

<div class="flex flex-col h-full">
  <!-- Header -->
  <div class="px-4 py-4 border-b border-stone-200">
    <h2 class="text-lg font-bold text-stone-800">Conversations</h2>
    <button
      class="mt-2 w-full px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
             font-medium text-sm hover:brightness-110 min-h-[44px]"
      onclick={onNewConversation}
    >
      + New conversation
    </button>
  </div>

  <!-- List -->
  <div class="flex-1 overflow-y-auto">
    {#if conversations.length === 0}
      <div class="px-4 py-8 text-center">
        <p class="text-sm text-stone-400">No conversations yet</p>
      </div>
    {:else}
      {#each conversations as conv (conv.id)}
        <div class="relative">
          <button
            class="w-full text-left px-4 py-3 border-b border-stone-100
                   hover:bg-stone-50 transition-colors min-h-[60px]
                   {activeConversationId === conv.id ? 'bg-stone-100' : ''}"
            onclick={() => onSelect(conv.id)}
          >
            <div class="flex items-start justify-between gap-2">
              <div class="flex-1 min-w-0">
                <p class="text-sm font-medium text-stone-800 truncate">{conv.title}</p>
                <p class="text-xs text-stone-500 truncate mt-0.5">{conv.last_message_preview}</p>
              </div>
              <span class="text-xs text-stone-400 flex-shrink-0">
                {relativeTime(conv.last_message_at)}
              </span>
            </div>
          </button>

          <!-- Delete confirmation -->
          {#if confirmDeleteId === conv.id}
            <div class="absolute inset-0 bg-white flex items-center justify-between px-4
                        border-b border-stone-100">
              <span class="text-xs text-stone-600">Delete this conversation?</span>
              <div class="flex gap-2">
                <button
                  class="px-3 py-1 text-xs text-stone-500 min-h-[32px]"
                  onclick={() => confirmDeleteId = null}
                >
                  Cancel
                </button>
                <button
                  class="px-3 py-1 text-xs text-red-600 font-medium min-h-[32px]"
                  onclick={() => { onDelete(conv.id); confirmDeleteId = null; }}
                >
                  Delete
                </button>
              </div>
            </div>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</div>
```

### ChatEmptyState

```svelte
<!-- src/lib/components/chat/ChatEmptyState.svelte -->
<script lang="ts">
  import type { PromptSuggestion } from '$lib/types/chat';

  interface Props {
    profileName: string;
    suggestions: PromptSuggestion[];
    onSuggestionTap: (suggestion: PromptSuggestion) => void;
  }
  let { profileName, suggestions, onSuggestionTap }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center h-full px-6 text-center max-w-md mx-auto">
  <!-- Coheara avatar (larger) -->
  <div class="w-16 h-16 rounded-full bg-[var(--color-primary)] flex items-center justify-center
              text-white text-2xl font-bold mb-4">
    C
  </div>

  <h2 class="text-xl font-bold text-stone-800 mb-2">
    Hello {profileName}!
  </h2>
  <p class="text-sm text-stone-500 mb-8 leading-relaxed">
    I can help you understand your medical documents. Ask me anything about your prescriptions,
    lab results, or health records.
  </p>

  <!-- Prompt suggestions -->
  {#if suggestions.length > 0}
    <div class="w-full">
      <p class="text-xs text-stone-400 uppercase font-medium mb-3">Try asking</p>
      <div class="grid grid-cols-1 gap-2">
        {#each suggestions as suggestion}
          <button
            class="w-full text-left px-4 py-3 rounded-xl bg-white border border-stone-200
                   text-sm text-stone-700 hover:border-[var(--color-primary)]
                   hover:shadow-sm transition-all min-h-[44px]"
            onclick={() => onSuggestionTap(suggestion)}
          >
            {suggestion.text}
          </button>
        {/each}
      </div>
    </div>
  {/if}
</div>
```

---

## [12] Frontend API

```typescript
// src/lib/api/chat.ts
import { invoke } from '@tauri-apps/api/core';
import type {
  Message,
  ConversationSummary,
  PromptSuggestion,
} from '$lib/types/chat';

/** Start a new conversation */
export async function startConversation(): Promise<string> {
  return invoke<string>('start_conversation');
}

/** Send a message (streaming response comes via Tauri events) */
export async function sendChatMessage(
  conversationId: string,
  text: string,
): Promise<void> {
  return invoke('send_chat_message', { conversationId, text });
}

/** Get all messages for a conversation */
export async function getConversationMessages(
  conversationId: string,
): Promise<Message[]> {
  return invoke<Message[]>('get_conversation_messages', { conversationId });
}

/** List all conversations for the current profile */
export async function listConversations(): Promise<ConversationSummary[]> {
  return invoke<ConversationSummary[]>('list_conversations');
}

/** Delete a conversation and all its messages */
export async function deleteConversation(conversationId: string): Promise<void> {
  return invoke('delete_conversation', { conversationId });
}

/** Save feedback for a message */
export async function setMessageFeedback(
  messageId: string,
  feedback: 'Helpful' | 'NotHelpful' | null,
): Promise<void> {
  return invoke('set_message_feedback', { messageId, feedback });
}

/** Get prompt suggestions based on profile data */
export async function getPromptSuggestions(): Promise<PromptSuggestion[]> {
  return invoke<PromptSuggestion[]>('get_prompt_suggestions');
}
```

---

## [13] Error Handling

User-facing error messages follow the calm design language:

| Error | User Message | Recovery |
|-------|-------------|----------|
| Ollama not running | "Coheara's AI assistant isn't available right now. Please make sure the application is fully started." | Retry button. Log `tracing::error!` with connection details. |
| No model available | "The AI model hasn't finished loading yet. This can take a minute after starting Coheara." | Retry button with 10-second delay suggestion. |
| No relevant context (NoContext) | "I don't have enough information in your documents to answer this question. Try loading more documents or rephrasing your question." | Show prompt suggestions as alternatives. |
| Streaming interrupted | "The response was interrupted. Please try asking again." | Retry button to re-send the same message. |
| Session expired | Redirected to profile unlock (ProfileGuard handles). | Re-enter password, conversation preserved. |
| Database error (message save) | "I couldn't save this message. Please try again." | Retry button. Message stays in local state but marked unsaved. |
| Conversation not found | "This conversation could not be loaded." | Navigate to conversation list. |
| Safety filter rejected response | Response is silently regenerated (up to 2 retries). If all fail: "I'm having trouble answering this question. Try rephrasing or asking something different." | Show prompt suggestions. |
| Feedback save failed | Silent failure. Log warning. Do not interrupt patient. | No user-visible error. |

All errors logged via `tracing::warn!` or `tracing::error!`. No patient query text in logs. Only conversation_id and error type.

---

## [14] Security

| Concern | Mitigation |
|---------|-----------|
| Patient queries in memory | Passed to RAG pipeline, then dropped. Never persisted in plaintext logs. |
| Message persistence | Content field encrypted in SQLite via L0-03 field-level encryption. Decrypted only with active ProfileSession. |
| Citations contain chunk_text | Chunk text comes from patient's own documents, already encrypted at rest. Decrypted only for display. |
| Prompt injection via patient input | Patient query placed in delimited section. System prompt instructs model to ignore instructions in queries. |
| Conversation accessible cross-profile | All queries filtered by `profile_id`. SQLite queries include `WHERE profile_id = ?` clause. |
| Streaming events visible to other windows | Tauri events scoped to single window. `conversation_id` filter on frontend as defense-in-depth. |
| Feedback data | Stays local. Never transmitted. Used only for local analytics. |
| Ollama communication | Localhost only (127.0.0.1). Not accessible from network. No TLS needed for loopback. |
| Activity tracking | Every Tauri command calls `state.update_activity()` to prevent false inactivity locks during conversation. |

---

## [15] Testing

### Unit Tests (Rust)

| Test | What |
|------|------|
| `test_start_conversation` | Creates conversation with valid UUID, correct profile_id, default title |
| `test_start_conversation_no_session` | Returns error when no active profile session |
| `test_send_chat_message_saves_patient` | Patient message saved in messages table with correct role and content |
| `test_send_chat_message_saves_response` | Coheara response saved with citations_json, confidence, and cleaned text |
| `test_send_chat_message_streams_tokens` | Stream events emitted with Token payloads before Done |
| `test_send_chat_message_streams_done` | Done event includes full_text, confidence, and boundary_check |
| `test_send_chat_message_no_context` | Returns graceful "no context" message when no documents exist |
| `test_get_conversation_messages_ordered` | Messages returned in timestamp ASC order |
| `test_get_conversation_messages_empty` | Returns empty vec for conversation with no messages |
| `test_get_conversation_messages_invalid_id` | Returns error for invalid UUID |
| `test_list_conversations_ordered` | Conversations returned in last_message_at DESC order |
| `test_list_conversations_profile_scoped` | Only returns conversations for current profile |
| `test_list_conversations_empty` | Returns empty vec for new profile |
| `test_delete_conversation_cascades` | Deleting conversation removes all its messages |
| `test_delete_conversation_wrong_profile` | Cannot delete another profile's conversation |
| `test_set_message_feedback_helpful` | Feedback saved as 'helpful' in messages table |
| `test_set_message_feedback_not_helpful` | Feedback saved as 'not_helpful' in messages table |
| `test_set_message_feedback_clear` | Feedback set to NULL when cleared |
| `test_generate_title_short` | Message under 50 chars used as-is |
| `test_generate_title_long` | Message over 50 chars truncated with "..." |
| `test_generate_title_unicode` | UTF-8 characters handled correctly (no mid-char split) |
| `test_get_prompt_suggestions_default` | Returns default suggestions when no data exists |
| `test_get_prompt_suggestions_contextual` | Returns medication suggestion when medications exist |
| `test_chunk_to_payload_token` | Non-final chunk maps to Token payload |
| `test_chunk_to_payload_final` | Final chunk returns None (handled separately) |
| `test_conversation_message_count_updated` | message_count incremented after each message |
| `test_conversation_last_message_at_updated` | last_message_at updated after each message |

### Frontend Tests

| Test | What |
|------|------|
| `test_chat_empty_state_rendered` | Empty state shows greeting with profile name |
| `test_chat_suggestions_shown` | Prompt suggestions rendered for empty conversation |
| `test_chat_suggestion_tap_sends` | Tapping suggestion fills input and sends message |
| `test_patient_message_right_aligned` | Patient messages have justify-end class |
| `test_coheara_message_left_aligned` | Coheara messages have avatar and left alignment |
| `test_streaming_cursor_visible` | Blinking cursor shown during streaming |
| `test_streaming_cursor_hidden` | Cursor removed when streaming completes |
| `test_streaming_indicator_shown` | "Searching your documents..." shown before first token |
| `test_citations_shown_after_streaming` | Citation chips not visible during streaming, visible after |
| `test_citation_chip_tap_opens_panel` | Tapping chip opens slide-up panel with excerpt |
| `test_citation_panel_view_document` | "View full document" button navigates to document detail |
| `test_confidence_well_supported` | Confidence >= 0.8 shows "Well supported" in green |
| `test_confidence_partially_supported` | Confidence 0.5-0.79 shows "Partially supported" in amber |
| `test_confidence_limited` | Confidence < 0.5 shows "Limited information" in stone |
| `test_feedback_thumbs_up` | Thumbs up saves 'Helpful' feedback |
| `test_feedback_thumbs_down` | Thumbs down saves 'NotHelpful' feedback |
| `test_feedback_deselect` | Tapping same button again clears feedback |
| `test_feedback_thank_you_fades` | "Thank you" text appears and fades after 2 seconds |
| `test_conversation_list_rendered` | Conversation list shows titles and previews |
| `test_conversation_switch` | Selecting conversation loads its messages |
| `test_new_conversation_clears_messages` | Starting new conversation clears message list |
| `test_send_button_disabled_empty` | Send button disabled when input is empty |
| `test_send_button_disabled_streaming` | Send button disabled during streaming |
| `test_error_message_displayed` | Stream error shown in amber bubble |
| `test_scroll_to_bottom_on_message` | Container scrolls to bottom on new message |
| `test_conversation_delete_confirmation` | Delete requires confirmation before executing |

---

## [16] Performance

| Metric | Target |
|--------|--------|
| Chat screen initial load | < 100ms (conversations list + suggestions) |
| Conversation messages load | < 150ms for 50 messages |
| Message send to first token | < 3 seconds (16GB RAM), < 5 seconds (8GB RAM) |
| Token-to-screen latency | < 50ms per token (Tauri event overhead) |
| Streaming frame rate | 60fps during token append (no jank) |
| Citation panel open | < 100ms animation |
| Conversation switch | < 200ms |
| Feedback save | < 100ms (async, non-blocking) |
| Conversation list refresh | < 100ms |
| Memory usage during streaming | < 5MB additional for streaming buffer |

### Performance Strategy

- Single Tauri command for message send (streaming response via events, not polling)
- Messages loaded once per conversation switch, not re-fetched on every render
- Local state updated optimistically (patient message shown before server confirms)
- Conversation list loaded once on mount, refreshed only after send/delete
- Citation panel uses CSS animation (GPU-accelerated `transform: translateY`)
- `requestAnimationFrame` for scroll-to-bottom to avoid layout thrashing

---

## [17] Open Questions

| # | Question | Status |
|---|---------|--------|
| OQ-01 | Should we support markdown rendering in Coheara responses? | Deferred to Phase 2. Plain text with whitespace preservation for Phase 1. |
| OQ-02 | Should conversations be exportable (PDF, text)? | Deferred to Phase 2. Useful for appointment preparation. |
| OQ-03 | Should streaming be cancellable mid-response? | Yes for Phase 1. Frontend stops listening; backend needs abort signal (requires async cancellation in Ollama client). Marking as P2 since it requires Ollama cancel API. |
| OQ-04 | Should the input support voice-to-text? | Deferred to Phase 2. Requires platform-specific speech recognition APIs. |
| OQ-05 | Maximum conversations per profile before performance degrades? | Need benchmarking. Estimated safe limit: 1000 conversations, 50 messages per conversation. Add pagination to conversation list if needed. |
| OQ-06 | Should we show typing speed / tokens-per-second to indicate model performance? | No. Too technical for patient audience. Keep the streaming animation as the only feedback. |
