//! CHAT-QUEUE-01: Chat queue service — in-memory job lifecycle manager.
//!
//! Pure state machine with no I/O. The worker loop (chat_queue_worker) drives
//! actual processing. Thread-safe via Mutex. Notify wakes the worker when
//! messages are enqueued.
//!
//! Pattern: Mirrors ImportQueueService (BTL-10 C3).
//! Signal/iMessage UX: persist immediately, process when SLM free, notify.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// State machine for a chat queue item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatQueueState {
    /// Waiting for SLM to become available.
    Queued,
    /// Butler lock requested — blocking until SLM free.
    Acquiring,
    /// Tokens flowing from RAG pipeline to frontend.
    Streaming,
    /// AI response persisted to DB. Terminal.
    Complete,
    /// Error occurred at any stage. Terminal.
    Failed,
}

impl ChatQueueState {
    /// Whether this state is terminal (no further transitions).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Complete | Self::Failed)
    }

    /// Valid next states from this state.
    fn valid_transitions(&self) -> &'static [ChatQueueState] {
        match self {
            Self::Queued => &[Self::Acquiring, Self::Failed],
            Self::Acquiring => &[Self::Streaming, Self::Failed],
            Self::Streaming => &[Self::Complete, Self::Failed],
            Self::Complete | Self::Failed => &[],
        }
    }

    /// Check if transitioning to `target` is valid.
    pub fn can_transition_to(&self, target: &ChatQueueState) -> bool {
        self.valid_transitions().contains(target)
    }
}

/// A single chat message in the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatQueueItem {
    /// Unique queue item ID (UUID).
    pub id: String,
    /// Conversation this message belongs to.
    pub conversation_id: String,
    /// Patient message ID (already persisted in messages table).
    pub patient_message_id: String,
    /// Sanitized patient text (for worker to process).
    pub text: String,
    /// Current state in the queue lifecycle.
    pub state: ChatQueueState,
    /// 1-indexed position assigned at enqueue time.
    pub queue_position: u32,
    /// Error message if state is Failed.
    pub error: Option<String>,
    /// When this item was enqueued (RFC3339).
    pub queued_at: String,
    /// When processing started (RFC3339).
    pub started_at: Option<String>,
    /// When processing completed or failed (RFC3339).
    pub completed_at: Option<String>,
}

/// Snapshot of the entire chat queue (for IPC serialization).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatQueueSnapshot {
    pub items: Vec<ChatQueueItem>,
    pub is_processing: bool,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// In-memory chat queue with message lifecycle management.
pub struct ChatQueueService {
    items: Mutex<Vec<ChatQueueItem>>,
    notify: tokio::sync::Notify,
    processing: AtomicBool,
}

impl ChatQueueService {
    pub fn new() -> Self {
        Self {
            items: Mutex::new(Vec::new()),
            notify: tokio::sync::Notify::new(),
            processing: AtomicBool::new(false),
        }
    }

    /// Enqueue a chat message for processing. Returns the queue item ID.
    ///
    /// The patient message must already be persisted in the messages table.
    /// This just tracks it for deferred SLM processing.
    pub fn enqueue(
        &self,
        conversation_id: String,
        patient_message_id: String,
        text: String,
    ) -> String {
        let mut items = self.items.lock().expect("chat queue lock poisoned");

        // Queue position: count of non-terminal items + 1
        let position = items.iter().filter(|i| !i.state.is_terminal()).count() as u32 + 1;

        let item = ChatQueueItem {
            id: Uuid::new_v4().to_string(),
            conversation_id,
            patient_message_id,
            text,
            state: ChatQueueState::Queued,
            queue_position: position,
            error: None,
            queued_at: Utc::now().to_rfc3339(),
            started_at: None,
            completed_at: None,
        };

        let id = item.id.clone();
        items.push(item);
        drop(items);

        self.notify.notify_one();
        id
    }

    /// Take the next queued item (transitions it to Acquiring).
    pub fn next_queued(&self) -> Option<ChatQueueItem> {
        let mut items = self.items.lock().expect("chat queue lock poisoned");
        let pos = items
            .iter()
            .position(|i| i.state == ChatQueueState::Queued)?;
        items[pos].state = ChatQueueState::Acquiring;
        items[pos].started_at = Some(Utc::now().to_rfc3339());
        Some(items[pos].clone())
    }

    /// Update the state of a queue item. Returns Err if item not found or
    /// transition invalid.
    pub fn update_state(
        &self,
        item_id: &str,
        new_state: ChatQueueState,
        error: Option<String>,
    ) -> Result<(), ChatQueueError> {
        let mut items = self.items.lock().expect("chat queue lock poisoned");
        let item = items
            .iter_mut()
            .find(|i| i.id == item_id)
            .ok_or(ChatQueueError::ItemNotFound)?;

        if !item.state.can_transition_to(&new_state) {
            return Err(ChatQueueError::InvalidTransition {
                from: format!("{:?}", item.state),
                to: format!("{:?}", new_state),
            });
        }

        item.state = new_state.clone();

        if let Some(err) = error {
            item.error = Some(err);
        }

        if new_state.is_terminal() {
            item.completed_at = Some(Utc::now().to_rfc3339());
        }

        Ok(())
    }

    /// Get a snapshot of all items.
    pub fn snapshot(&self) -> ChatQueueSnapshot {
        let items = self.items.lock().expect("chat queue lock poisoned");
        ChatQueueSnapshot {
            items: items.clone(),
            is_processing: self.processing.load(Ordering::Relaxed),
        }
    }

    /// Get a single item by ID.
    pub fn get_item(&self, item_id: &str) -> Option<ChatQueueItem> {
        let items = self.items.lock().expect("chat queue lock poisoned");
        items.iter().find(|i| i.id == item_id).cloned()
    }

    /// Count of non-terminal items.
    pub fn pending_count(&self) -> usize {
        let items = self.items.lock().expect("chat queue lock poisoned");
        items.iter().filter(|i| !i.state.is_terminal()).count()
    }

    /// Get pending items for a specific conversation.
    pub fn pending_for_conversation(&self, conversation_id: &str) -> Vec<ChatQueueItem> {
        let items = self.items.lock().expect("chat queue lock poisoned");
        items
            .iter()
            .filter(|i| i.conversation_id == conversation_id && !i.state.is_terminal())
            .cloned()
            .collect()
    }

    /// Get a reference to the Notify handle (for the worker to await).
    pub fn notifier(&self) -> &tokio::sync::Notify {
        &self.notify
    }

    /// Set whether the worker is actively processing.
    pub fn set_processing(&self, processing: bool) {
        self.processing.store(processing, Ordering::Relaxed);
    }

    /// Check if the worker is actively processing.
    pub fn is_processing(&self) -> bool {
        self.processing.load(Ordering::Relaxed)
    }

    /// Reset the queue (F7: profile switch security).
    pub fn reset(&self) {
        let mut items = self.items.lock().expect("chat queue lock poisoned");
        items.clear();
        self.processing.store(false, Ordering::Relaxed);
    }

    /// Remove oldest terminal items beyond the most recent `keep` count.
    ///
    /// Called by the worker after each item completes. Prevents unbounded
    /// memory growth over long sessions (FIX-5). Items are removed oldest
    /// first (front of the Vec — terminal items are always earlier than
    /// active items because the worker processes sequentially).
    pub fn prune_terminal(&self, keep: usize) {
        let mut items = self.items.lock().expect("chat queue lock poisoned");
        let terminal_count = items.iter().filter(|i| i.state.is_terminal()).count();
        if terminal_count <= keep {
            return;
        }
        let to_remove = terminal_count - keep;
        let mut removed = 0;
        items.retain(|i| {
            if removed >= to_remove {
                return true;
            }
            if i.state.is_terminal() {
                removed += 1;
                return false;
            }
            true
        });
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatQueueError {
    ItemNotFound,
    InvalidTransition { from: String, to: String },
}

impl std::fmt::Display for ChatQueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ItemNotFound => write!(f, "Queue item not found"),
            Self::InvalidTransition { from, to } => {
                write!(f, "Invalid state transition: {from} → {to}")
            }
        }
    }
}

impl std::error::Error for ChatQueueError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> ChatQueueService {
        ChatQueueService::new()
    }

    // -- State machine --

    #[test]
    fn queued_can_transition_to_acquiring() {
        assert!(ChatQueueState::Queued.can_transition_to(&ChatQueueState::Acquiring));
    }

    #[test]
    fn queued_can_transition_to_failed() {
        assert!(ChatQueueState::Queued.can_transition_to(&ChatQueueState::Failed));
    }

    #[test]
    fn queued_cannot_transition_to_streaming() {
        assert!(!ChatQueueState::Queued.can_transition_to(&ChatQueueState::Streaming));
    }

    #[test]
    fn queued_cannot_transition_to_complete() {
        assert!(!ChatQueueState::Queued.can_transition_to(&ChatQueueState::Complete));
    }

    #[test]
    fn acquiring_can_transition_to_streaming() {
        assert!(ChatQueueState::Acquiring.can_transition_to(&ChatQueueState::Streaming));
    }

    #[test]
    fn acquiring_can_transition_to_failed() {
        assert!(ChatQueueState::Acquiring.can_transition_to(&ChatQueueState::Failed));
    }

    #[test]
    fn streaming_can_transition_to_complete() {
        assert!(ChatQueueState::Streaming.can_transition_to(&ChatQueueState::Complete));
    }

    #[test]
    fn streaming_can_transition_to_failed() {
        assert!(ChatQueueState::Streaming.can_transition_to(&ChatQueueState::Failed));
    }

    #[test]
    fn complete_is_terminal() {
        assert!(ChatQueueState::Complete.is_terminal());
        assert!(ChatQueueState::Complete.valid_transitions().is_empty());
    }

    #[test]
    fn failed_is_terminal() {
        assert!(ChatQueueState::Failed.is_terminal());
        assert!(ChatQueueState::Failed.valid_transitions().is_empty());
    }

    // -- Enqueue --

    #[test]
    fn enqueue_creates_queued_item() {
        let svc = service();
        let id = svc.enqueue(
            "conv-1".into(),
            "msg-1".into(),
            "What are my medications?".into(),
        );

        let snap = svc.snapshot();
        assert_eq!(snap.items.len(), 1);
        assert_eq!(snap.items[0].id, id);
        assert_eq!(snap.items[0].state, ChatQueueState::Queued);
        assert_eq!(snap.items[0].conversation_id, "conv-1");
        assert_eq!(snap.items[0].patient_message_id, "msg-1");
        assert_eq!(snap.items[0].text, "What are my medications?");
        assert_eq!(snap.items[0].queue_position, 1);
        assert!(snap.items[0].error.is_none());
        assert!(snap.items[0].started_at.is_none());
        assert!(snap.items[0].completed_at.is_none());
    }

    #[test]
    fn enqueue_preserves_order() {
        let svc = service();
        let id1 = svc.enqueue("conv-1".into(), "msg-1".into(), "Q1".into());
        let id2 = svc.enqueue("conv-1".into(), "msg-2".into(), "Q2".into());
        let id3 = svc.enqueue("conv-2".into(), "msg-3".into(), "Q3".into());

        let snap = svc.snapshot();
        assert_eq!(snap.items.len(), 3);
        assert_eq!(snap.items[0].id, id1);
        assert_eq!(snap.items[1].id, id2);
        assert_eq!(snap.items[2].id, id3);
    }

    #[test]
    fn enqueue_assigns_incrementing_positions() {
        let svc = service();
        svc.enqueue("conv-1".into(), "msg-1".into(), "Q1".into());
        svc.enqueue("conv-1".into(), "msg-2".into(), "Q2".into());
        svc.enqueue("conv-1".into(), "msg-3".into(), "Q3".into());

        let snap = svc.snapshot();
        assert_eq!(snap.items[0].queue_position, 1);
        assert_eq!(snap.items[1].queue_position, 2);
        assert_eq!(snap.items[2].queue_position, 3);
    }

    // -- next_queued --

    #[test]
    fn next_queued_transitions_to_acquiring() {
        let svc = service();
        svc.enqueue("conv-1".into(), "msg-1".into(), "Q1".into());

        let item = svc.next_queued().unwrap();
        assert_eq!(item.state, ChatQueueState::Acquiring);
        assert!(item.started_at.is_some());
    }

    #[test]
    fn next_queued_skips_non_queued() {
        let svc = service();
        let id1 = svc.enqueue("conv-1".into(), "msg-1".into(), "Q1".into());
        svc.enqueue("conv-1".into(), "msg-2".into(), "Q2".into());

        // Take first
        svc.next_queued().unwrap();
        // Fail first
        svc.update_state(&id1, ChatQueueState::Failed, Some("err".into()))
            .unwrap();

        // Second should be next
        let item = svc.next_queued().unwrap();
        assert_eq!(item.text, "Q2");
    }

    #[test]
    fn next_queued_empty_returns_none() {
        let svc = service();
        assert!(svc.next_queued().is_none());
    }

    // -- update_state --

    #[test]
    fn update_state_full_lifecycle() {
        let svc = service();
        let id = svc.enqueue("conv-1".into(), "msg-1".into(), "Q1".into());
        svc.next_queued(); // Queued → Acquiring

        svc.update_state(&id, ChatQueueState::Streaming, None)
            .unwrap();
        let item = svc.get_item(&id).unwrap();
        assert_eq!(item.state, ChatQueueState::Streaming);
        assert!(item.completed_at.is_none());

        svc.update_state(&id, ChatQueueState::Complete, None)
            .unwrap();
        let item = svc.get_item(&id).unwrap();
        assert_eq!(item.state, ChatQueueState::Complete);
        assert!(item.completed_at.is_some());
    }

    #[test]
    fn update_state_failed_with_error() {
        let svc = service();
        let id = svc.enqueue("conv-1".into(), "msg-1".into(), "Q1".into());
        svc.next_queued();

        svc.update_state(
            &id,
            ChatQueueState::Failed,
            Some("Ollama unreachable".into()),
        )
        .unwrap();
        let item = svc.get_item(&id).unwrap();
        assert_eq!(item.state, ChatQueueState::Failed);
        assert_eq!(item.error.as_deref(), Some("Ollama unreachable"));
        assert!(item.completed_at.is_some());
    }

    #[test]
    fn update_nonexistent_returns_error() {
        let svc = service();
        assert_eq!(
            svc.update_state("nonexistent", ChatQueueState::Complete, None)
                .unwrap_err(),
            ChatQueueError::ItemNotFound,
        );
    }

    #[test]
    fn update_invalid_transition_rejected() {
        let svc = service();
        let id = svc.enqueue("conv-1".into(), "msg-1".into(), "Q1".into());

        // Queued → Complete is invalid (must go through Acquiring → Streaming)
        let err = svc
            .update_state(&id, ChatQueueState::Complete, None)
            .unwrap_err();
        assert!(matches!(err, ChatQueueError::InvalidTransition { .. }));

        // Complete → Queued is invalid (terminal)
        svc.next_queued();
        svc.update_state(&id, ChatQueueState::Streaming, None)
            .unwrap();
        svc.update_state(&id, ChatQueueState::Complete, None)
            .unwrap();
        let err = svc
            .update_state(&id, ChatQueueState::Queued, None)
            .unwrap_err();
        assert!(matches!(err, ChatQueueError::InvalidTransition { .. }));
    }

    // -- pending_count --

    #[test]
    fn pending_count_excludes_terminal() {
        let svc = service();
        let id1 = svc.enqueue("conv-1".into(), "msg-1".into(), "Q1".into());
        svc.enqueue("conv-1".into(), "msg-2".into(), "Q2".into());
        assert_eq!(svc.pending_count(), 2);

        svc.next_queued();
        svc.update_state(&id1, ChatQueueState::Streaming, None)
            .unwrap();
        svc.update_state(&id1, ChatQueueState::Complete, None)
            .unwrap();
        assert_eq!(svc.pending_count(), 1);
    }

    // -- pending_for_conversation --

    #[test]
    fn pending_for_conversation_filters_by_conv_id() {
        let svc = service();
        svc.enqueue("conv-1".into(), "msg-1".into(), "Q1".into());
        svc.enqueue("conv-2".into(), "msg-2".into(), "Q2".into());
        svc.enqueue("conv-1".into(), "msg-3".into(), "Q3".into());

        let conv1_items = svc.pending_for_conversation("conv-1");
        assert_eq!(conv1_items.len(), 2);
        assert!(conv1_items.iter().all(|i| i.conversation_id == "conv-1"));

        let conv2_items = svc.pending_for_conversation("conv-2");
        assert_eq!(conv2_items.len(), 1);

        let conv3_items = svc.pending_for_conversation("conv-3");
        assert!(conv3_items.is_empty());
    }

    #[test]
    fn pending_for_conversation_excludes_terminal() {
        let svc = service();
        let id = svc.enqueue("conv-1".into(), "msg-1".into(), "Q1".into());
        svc.enqueue("conv-1".into(), "msg-2".into(), "Q2".into());

        svc.next_queued();
        svc.update_state(&id, ChatQueueState::Streaming, None)
            .unwrap();
        svc.update_state(&id, ChatQueueState::Complete, None)
            .unwrap();

        let pending = svc.pending_for_conversation("conv-1");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].text, "Q2");
    }

    // -- snapshot --

    #[test]
    fn snapshot_reflects_state() {
        let svc = service();
        svc.enqueue("conv-1".into(), "msg-1".into(), "Q1".into());
        svc.enqueue("conv-1".into(), "msg-2".into(), "Q2".into());

        let snap = svc.snapshot();
        assert_eq!(snap.items.len(), 2);
        assert!(!snap.is_processing);
    }

    // -- processing flag --

    #[test]
    fn processing_flag() {
        let svc = service();
        assert!(!svc.is_processing());

        svc.set_processing(true);
        assert!(svc.is_processing());

        svc.set_processing(false);
        assert!(!svc.is_processing());
    }

    // -- reset --

    #[test]
    fn reset_clears_all() {
        let svc = service();
        svc.enqueue("conv-1".into(), "msg-1".into(), "Q1".into());
        svc.enqueue("conv-1".into(), "msg-2".into(), "Q2".into());
        svc.set_processing(true);

        svc.reset();

        let snap = svc.snapshot();
        assert!(snap.items.is_empty());
        assert!(!snap.is_processing);
    }

    // -- prune_terminal --

    #[test]
    fn prune_terminal_removes_oldest() {
        let svc = service();
        // Create 5 items and complete them all
        let ids: Vec<String> = (0..5)
            .map(|i| svc.enqueue("conv-1".into(), format!("msg-{i}"), format!("Q{i}")))
            .collect();
        for id in &ids {
            svc.next_queued();
            svc.update_state(id, ChatQueueState::Streaming, None).unwrap();
            svc.update_state(id, ChatQueueState::Complete, None).unwrap();
        }
        assert_eq!(svc.snapshot().items.len(), 5);

        // Prune keeping 3 → should remove 2 oldest
        svc.prune_terminal(3);
        let snap = svc.snapshot();
        assert_eq!(snap.items.len(), 3);
        // Remaining should be the 3 newest (Q2, Q3, Q4)
        assert_eq!(snap.items[0].text, "Q2");
        assert_eq!(snap.items[1].text, "Q3");
        assert_eq!(snap.items[2].text, "Q4");
    }

    #[test]
    fn prune_terminal_preserves_active_items() {
        let svc = service();
        // Complete 3 items
        for i in 0..3 {
            let id = svc.enqueue("conv-1".into(), format!("msg-{i}"), format!("Done{i}"));
            svc.next_queued();
            svc.update_state(&id, ChatQueueState::Streaming, None).unwrap();
            svc.update_state(&id, ChatQueueState::Complete, None).unwrap();
        }
        // Add 2 active (Queued) items
        svc.enqueue("conv-1".into(), "msg-active1".into(), "Active1".into());
        svc.enqueue("conv-1".into(), "msg-active2".into(), "Active2".into());
        assert_eq!(svc.snapshot().items.len(), 5);

        // Prune terminal keeping 1 → removes 2 terminal, preserves 2 active
        svc.prune_terminal(1);
        let snap = svc.snapshot();
        assert_eq!(snap.items.len(), 3); // 1 terminal + 2 active
        assert_eq!(snap.items.iter().filter(|i| i.state.is_terminal()).count(), 1);
        assert_eq!(snap.items.iter().filter(|i| !i.state.is_terminal()).count(), 2);
    }

    #[test]
    fn prune_terminal_noop_when_under_limit() {
        let svc = service();
        let id = svc.enqueue("conv-1".into(), "msg-1".into(), "Q1".into());
        svc.next_queued();
        svc.update_state(&id, ChatQueueState::Streaming, None).unwrap();
        svc.update_state(&id, ChatQueueState::Complete, None).unwrap();

        svc.prune_terminal(50); // keep 50, only 1 terminal → noop
        assert_eq!(svc.snapshot().items.len(), 1);
    }

    // -- concurrent access --

    #[test]
    fn concurrent_enqueue() {
        let svc = std::sync::Arc::new(service());
        let mut handles = Vec::new();

        for i in 0..10 {
            let svc = svc.clone();
            handles.push(std::thread::spawn(move || {
                svc.enqueue(
                    format!("conv-{i}"),
                    format!("msg-{i}"),
                    format!("Question {i}"),
                );
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(svc.snapshot().items.len(), 10);
    }

    // -- serialization --

    #[test]
    fn chat_queue_state_serializes() {
        let json = serde_json::to_string(&ChatQueueState::Queued).unwrap();
        assert!(json.contains("Queued"));

        let json = serde_json::to_string(&ChatQueueState::Acquiring).unwrap();
        assert!(json.contains("Acquiring"));

        let json = serde_json::to_string(&ChatQueueState::Streaming).unwrap();
        assert!(json.contains("Streaming"));

        let json = serde_json::to_string(&ChatQueueState::Complete).unwrap();
        assert!(json.contains("Complete"));

        let json = serde_json::to_string(&ChatQueueState::Failed).unwrap();
        assert!(json.contains("Failed"));
    }

    #[test]
    fn chat_queue_item_serializes() {
        let item = ChatQueueItem {
            id: "abc-123".into(),
            conversation_id: "conv-1".into(),
            patient_message_id: "msg-1".into(),
            text: "What are my medications?".into(),
            state: ChatQueueState::Queued,
            queue_position: 1,
            error: None,
            queued_at: "2026-03-02T10:00:00Z".into(),
            started_at: None,
            completed_at: None,
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("abc-123"));
        assert!(json.contains("conv-1"));
        assert!(json.contains("msg-1"));
        assert!(json.contains("Queued"));
    }

    #[test]
    fn chat_queue_snapshot_serializes() {
        let snap = ChatQueueSnapshot {
            items: vec![],
            is_processing: true,
        };
        let json = serde_json::to_string(&snap).unwrap();
        assert!(json.contains("is_processing"));
        assert!(json.contains("true"));
    }
}
