//! Trait definitions for the night batch extraction pipeline.
//!
//! Four traits define the module boundaries (EP-2: domain-agnostic orchestrator):
//! - BatchScheduler: when to run + conversation eligibility
//! - ConversationAnalyzer: which domains to extract from which conversations
//! - DomainExtractor: domain-specific extraction logic
//! - PendingReviewStore: CRUD for pending review items

use rusqlite::Connection;

use super::error::ExtractionError;
use super::types::*;

/// M1: Determines when extraction should run and which conversations are eligible.
pub trait BatchScheduler: Send + Sync {
    /// Check if any conversations are eligible for extraction.
    fn has_pending_work(&self, conn: &Connection) -> Result<bool, ExtractionError>;

    /// Get all eligible conversations with their messages loaded.
    fn get_eligible_conversations(
        &self,
        conn: &Connection,
        config: &ExtractionConfig,
    ) -> Result<Vec<ConversationBatch>, ExtractionError>;

    /// Mark a conversation as extracted (prevents re-extraction until new messages).
    fn mark_extracted(
        &self,
        conn: &Connection,
        conversation_id: &str,
        batch_id: &str,
        domains_found: &[ExtractionDomain],
        items_count: u32,
        model_name: &str,
        duration_ms: u64,
    ) -> Result<(), ExtractionError>;
}

/// M2: Analyzes conversations to determine which domains contain extractable data.
/// Rule-based only — no LLM calls.
pub trait ConversationAnalyzer: Send + Sync {
    /// Analyze a conversation for extractable health domains.
    /// Returns which domains are present and which messages are signals.
    fn analyze(&self, conversation: &ConversationBatch) -> AnalysisResult;
}

/// M3: Domain-specific extraction from natural language.
/// Each domain (symptom, medication, appointment) implements this trait.
pub trait DomainExtractor: Send + Sync {
    /// Which domain this extractor handles.
    fn domain(&self) -> ExtractionDomain;

    /// Build the extraction prompt for this domain.
    fn build_prompt(&self, input: &ExtractionInput) -> String;

    /// Parse the LLM response into extracted items.
    fn parse_response(&self, response: &str) -> Result<Vec<ExtractedItem>, ExtractionError>;

    /// Validate extracted items for plausibility.
    fn validate(&self, items: &[ExtractedItem]) -> ValidationResult;

    /// Consolidate duplicate items within a single conversation.
    fn consolidate(&self, items: Vec<ExtractedItem>) -> Vec<ExtractedItem>;
}

/// M4: CRUD operations for the extraction_pending table.
pub trait PendingReviewStore: Send + Sync {
    /// Store multiple pending items from a batch run.
    fn store_pending(
        &self,
        conn: &Connection,
        items: &[PendingReviewItem],
    ) -> Result<(), ExtractionError>;

    /// Get all pending items (status = 'pending') for display on Home.
    fn get_pending(
        &self,
        conn: &Connection,
    ) -> Result<Vec<PendingReviewItem>, ExtractionError>;

    /// Get count of pending items.
    fn get_pending_count(
        &self,
        conn: &Connection,
    ) -> Result<u32, ExtractionError>;

    /// Mark an item as confirmed and return its data for dispatch.
    fn confirm_item(
        &self,
        conn: &Connection,
        item_id: &str,
    ) -> Result<PendingReviewItem, ExtractionError>;

    /// Mark an item as edited+confirmed with updated data.
    fn confirm_item_with_edits(
        &self,
        conn: &Connection,
        item_id: &str,
        edits: serde_json::Value,
    ) -> Result<PendingReviewItem, ExtractionError>;

    /// Mark an item as dismissed.
    fn dismiss_item(
        &self,
        conn: &Connection,
        item_id: &str,
    ) -> Result<(), ExtractionError>;

    /// Dismiss multiple items at once.
    fn dismiss_items(
        &self,
        conn: &Connection,
        item_ids: &[String],
    ) -> Result<(), ExtractionError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verify traits are object-safe (can be used as `dyn Trait`)
    #[test]
    fn traits_are_object_safe() {
        fn _assert_scheduler(_: &dyn BatchScheduler) {}
        fn _assert_analyzer(_: &dyn ConversationAnalyzer) {}
        fn _assert_extractor(_: &dyn DomainExtractor) {}
        fn _assert_store(_: &dyn PendingReviewStore) {}
    }
}
