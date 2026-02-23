//! Core types for the night batch extraction pipeline.
//!
//! These types model the full lifecycle:
//! Conversation → Analysis → Extraction → Verification → Pending Review → Dispatch.

use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════
// Domain Enum
// ═══════════════════════════════════════════

/// The three extractable domains from conversations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionDomain {
    Symptom,
    Medication,
    Appointment,
}

impl ExtractionDomain {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Symptom => "symptom",
            Self::Medication => "medication",
            Self::Appointment => "appointment",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "symptom" => Some(Self::Symptom),
            "medication" => Some(Self::Medication),
            "appointment" => Some(Self::Appointment),
            _ => None,
        }
    }

    pub fn all() -> &'static [ExtractionDomain] {
        &[Self::Symptom, Self::Medication, Self::Appointment]
    }
}

impl std::fmt::Display for ExtractionDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ═══════════════════════════════════════════
// Conversation Batch (input to analyzer)
// ═══════════════════════════════════════════

/// A conversation eligible for batch extraction.
/// Loaded from DB by the scheduler.
#[derive(Debug, Clone)]
pub struct ConversationBatch {
    pub id: String,
    pub title: Option<String>,
    pub messages: Vec<ConversationMessage>,
    pub last_message_at: NaiveDateTime,
    pub message_count: u32,
}

/// A single message within a conversation batch.
#[derive(Debug, Clone)]
pub struct ConversationMessage {
    pub id: String,
    pub index: usize,
    pub role: String,
    pub content: String,
    pub created_at: NaiveDateTime,
    /// Set by the analyzer: true if this message triggered domain classification.
    pub is_signal: bool,
}

// ═══════════════════════════════════════════
// Analysis Result (output of M2: Analyzer)
// ═══════════════════════════════════════════

/// Result of analyzing a conversation for extractable domains.
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    /// Domains detected in this conversation, with relevant message indices.
    pub domains: Vec<DomainMatch>,
    /// True if no health data detected (pure Q&A conversation).
    pub is_pure_qa: bool,
}

/// A domain match within a conversation.
#[derive(Debug, Clone)]
pub struct DomainMatch {
    pub domain: ExtractionDomain,
    /// Indices of messages that contain signal for this domain.
    pub signal_message_indices: Vec<usize>,
    /// Confidence of domain detection (0.0-1.0).
    pub detection_confidence: f32,
}

impl AnalysisResult {
    /// Build an ExtractionInput for a specific domain match.
    pub fn build_input(
        &self,
        conversation: &ConversationBatch,
        domain_match: &DomainMatch,
        patient_context: PatientContext,
        conversation_date: NaiveDate,
    ) -> ExtractionInput {
        // Include signal messages + 1 surrounding message for context.
        let mut selected_indices: Vec<usize> = Vec::new();
        for &idx in &domain_match.signal_message_indices {
            if idx > 0 {
                selected_indices.push(idx - 1);
            }
            selected_indices.push(idx);
            if idx + 1 < conversation.messages.len() {
                selected_indices.push(idx + 1);
            }
        }
        selected_indices.sort_unstable();
        selected_indices.dedup();

        let messages: Vec<ConversationMessage> = selected_indices
            .iter()
            .filter_map(|&idx| conversation.messages.get(idx))
            .cloned()
            .collect();

        ExtractionInput {
            conversation_id: conversation.id.clone(),
            messages,
            patient_context,
            conversation_date,
        }
    }
}

// ═══════════════════════════════════════════
// Extraction Input (input to M3: Engine)
// ═══════════════════════════════════════════

/// Input for a single domain extraction call.
#[derive(Debug, Clone)]
pub struct ExtractionInput {
    pub conversation_id: String,
    pub messages: Vec<ConversationMessage>,
    pub patient_context: PatientContext,
    pub conversation_date: NaiveDate,
}

/// Patient context for disambiguation.
#[derive(Debug, Clone, Default)]
pub struct PatientContext {
    pub active_medications: Vec<ActiveMedicationSummary>,
    pub recent_symptoms: Vec<RecentSymptomSummary>,
    pub known_allergies: Vec<String>,
    pub known_professionals: Vec<ProfessionalSummary>,
    pub date_of_birth: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveMedicationSummary {
    pub name: String,
    pub dose: String,
    pub frequency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentSymptomSummary {
    pub category: String,
    pub specific: String,
    pub severity: i32,
    pub onset_date: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfessionalSummary {
    pub name: String,
    pub specialty: Option<String>,
}

// ═══════════════════════════════════════════
// Extracted Item (output of M3: Engine)
// ═══════════════════════════════════════════

/// A single extracted item from any domain.
/// The `data` field holds domain-specific JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedItem {
    pub domain: ExtractionDomain,
    pub data: serde_json::Value,
    pub confidence: f32,
    pub source_message_indices: Vec<usize>,
}

// ═══════════════════════════════════════════
// Domain-specific extracted data structures
// ═══════════════════════════════════════════

/// Custom deserializer for source_messages that accepts both integers and
/// strings like "Msg 0" (MEDGEMMA-BENCHMARK-03 F4: inconsistent indexing).
fn deserialize_flexible_source_messages<'de, D>(deserializer: D) -> Result<Vec<usize>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct FlexibleVec;

    impl<'de> de::Visitor<'de> for FlexibleVec {
        type Value = Vec<usize>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("an array of integers or strings like \"Msg 0\"")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Vec<usize>, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut result = Vec::new();
            while let Some(val) = seq.next_element::<serde_json::Value>()? {
                if let Some(n) = val.as_u64() {
                    result.push(n as usize);
                } else if let Some(s) = val.as_str() {
                    let trimmed = s.trim();
                    if let Some(num_str) = trimmed
                        .strip_prefix("Msg ")
                        .or_else(|| trimmed.strip_prefix("msg "))
                    {
                        if let Ok(n) = num_str.trim().parse::<usize>() {
                            result.push(n);
                        }
                    } else if let Ok(n) = trimmed.parse::<usize>() {
                        result.push(n);
                    }
                }
            }
            Ok(result)
        }
    }

    deserializer.deserialize_seq(FlexibleVec)
}

/// Symptom extracted from conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedSymptomData {
    pub category: String,
    pub specific: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity_hint: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onset_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character: Option<String>,
    #[serde(default)]
    pub aggravating: Vec<String>,
    #[serde(default)]
    pub relieving: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing_pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_medication_hint: Option<String>,
    #[serde(default, deserialize_with = "deserialize_flexible_source_messages")]
    pub source_messages: Vec<usize>,
}

/// Medication extracted from conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedMedicationData {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dose: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub is_otc: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adherence_note: Option<String>,
    #[serde(default, deserialize_with = "deserialize_flexible_source_messages")]
    pub source_messages: Vec<usize>,
}

/// Appointment extracted from conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedAppointmentData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub professional_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub specialty: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, deserialize_with = "deserialize_flexible_source_messages")]
    pub source_messages: Vec<usize>,
}

// ═══════════════════════════════════════════
// Validation Result (output of validator)
// ═══════════════════════════════════════════

/// Result of validating extracted items.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub items: Vec<ExtractedItem>,
    pub warnings: Vec<String>,
    pub rejected_count: usize,
}

// ═══════════════════════════════════════════
// Grounding Assessment
// ═══════════════════════════════════════════

/// How well an extracted item is grounded in the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grounding {
    /// Key terms verified in source messages.
    Grounded,
    /// Some terms verified, some inferred.
    Partial,
    /// Cannot verify against source text.
    Ungrounded,
}

impl Grounding {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Grounded => "grounded",
            Self::Partial => "partial",
            Self::Ungrounded => "ungrounded",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "grounded" => Some(Self::Grounded),
            "partial" => Some(Self::Partial),
            "ungrounded" => Some(Self::Ungrounded),
            _ => None,
        }
    }
}

impl std::fmt::Display for Grounding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ═══════════════════════════════════════════
// Duplicate Detection
// ═══════════════════════════════════════════

/// Result of checking if an extracted item duplicates an existing DB record.
#[derive(Debug, Clone)]
pub enum DuplicateStatus {
    /// No duplicate found.
    New,
    /// Exact duplicate — skip extraction.
    AlreadyTracked { existing_id: String },
    /// Similar but not exact — show as warning.
    PossibleDuplicate { existing_id: String },
}

// ═══════════════════════════════════════════
// Pending Review Item (output of M4: Store)
// ═══════════════════════════════════════════

/// An item awaiting user review on the Home screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingReviewItem {
    pub id: String,
    pub conversation_id: String,
    pub batch_id: String,
    pub domain: ExtractionDomain,
    pub extracted_data: serde_json::Value,
    pub confidence: f32,
    pub grounding: Grounding,
    pub duplicate_of: Option<String>,
    pub source_message_ids: Vec<String>,
    /// Excerpt from the triggering conversation message(s) (LP-01 REV-12).
    pub source_quote: Option<String>,
    pub status: PendingStatus,
    pub created_at: String,
    pub reviewed_at: Option<String>,
}

/// Status of a pending review item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingStatus {
    Pending,
    Confirmed,
    EditedConfirmed,
    Dismissed,
}

impl PendingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Confirmed => "confirmed",
            Self::EditedConfirmed => "edited_confirmed",
            Self::Dismissed => "dismissed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "confirmed" => Some(Self::Confirmed),
            "edited_confirmed" => Some(Self::EditedConfirmed),
            "dismissed" => Some(Self::Dismissed),
            _ => None,
        }
    }
}

// ═══════════════════════════════════════════
// Frontend View (sent over IPC)
// ═══════════════════════════════════════════

/// Frontend-facing view of a pending extraction item.
/// Enriched with display-ready fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingExtractionView {
    pub id: String,
    pub domain: ExtractionDomain,
    pub confidence: f32,
    pub grounding: Grounding,
    pub fields: serde_json::Value,
    pub source_quote: String,
    pub source_conversation_title: String,
    pub duplicate_warning: Option<String>,
    pub created_at: String,
}

// ═══════════════════════════════════════════
// Batch Result (output of BatchRunner)
// ═══════════════════════════════════════════

/// Result of running a full extraction batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub conversations_processed: u32,
    pub conversations_skipped: u32,
    pub items_extracted: u32,
    pub items_stored: u32,
    pub duration_ms: u64,
    pub errors: Vec<String>,
}

impl BatchResult {
    pub fn empty() -> Self {
        Self {
            conversations_processed: 0,
            conversations_skipped: 0,
            items_extracted: 0,
            items_stored: 0,
            duration_ms: 0,
            errors: Vec::new(),
        }
    }
}

// ═══════════════════════════════════════════
// Batch Status Events (Tauri events)
// ═══════════════════════════════════════════

/// Event emitted during batch processing for the header indicator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BatchStatusEvent {
    Started {
        conversation_count: u32,
    },
    Progress {
        completed: u32,
        total: u32,
        current_title: String,
    },
    Completed {
        items_found: u32,
        duration_ms: u64,
    },
    Failed {
        error: String,
    },
}

// ═══════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════

/// Configuration for the extraction pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionConfig {
    /// Model to use for extraction (e.g., "medgemma:4b").
    pub model_name: String,
    /// Minimum confidence to store an extracted item (0.0-1.0).
    /// MEDGEMMA-BENCHMARK-03 F7 validated 0.7: grounded items score 0.8+,
    /// partially-grounded items score ~0.6, hallucinated items score lower.
    pub confidence_threshold: f32,
    /// Hours between batch runs (default: 6).
    pub batch_cooldown_hours: u32,
    /// User-configured hour to start batch (0-23, None = any idle time).
    pub batch_start_hour: Option<u32>,
    /// Minutes of inactivity before batch can run.
    pub idle_minutes: u32,
    /// Maximum conversations per batch (prevents runaway processing).
    pub max_conversations_per_batch: u32,
    /// Maximum items extracted per domain per conversation.
    pub max_items_per_domain: u32,
    /// Hours a conversation must be "cold" before extraction.
    pub cold_hours: u32,
}

impl Default for ExtractionConfig {
    fn default() -> Self {
        Self {
            model_name: "medgemma:4b".to_string(),
            confidence_threshold: 0.7,
            batch_cooldown_hours: 6,
            batch_start_hour: Some(2), // Default: 2:00 AM
            idle_minutes: 5,
            max_conversations_per_batch: 20,
            max_items_per_domain: 5,
            cold_hours: 6,
        }
    }
}

// ═══════════════════════════════════════════
// Dispatch Result (after user confirms)
// ═══════════════════════════════════════════

/// Result of dispatching a confirmed item to the actual database table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchResult {
    pub item_id: String,
    pub domain: ExtractionDomain,
    pub success: bool,
    pub created_record_id: Option<String>,
    pub error: Option<String>,
    /// Temporal correlations found after symptom dispatch (medication changes near onset date).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlations: Option<Vec<crate::journal::TemporalCorrelation>>,
    /// Warning if a duplicate record was found for this item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate_warning: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extraction_domain_roundtrip() {
        for domain in ExtractionDomain::all() {
            let s = domain.as_str();
            let parsed = ExtractionDomain::from_str(s);
            assert_eq!(parsed, Some(*domain), "Roundtrip failed for {s}");
        }
    }

    #[test]
    fn extraction_domain_display() {
        assert_eq!(ExtractionDomain::Symptom.to_string(), "symptom");
        assert_eq!(ExtractionDomain::Medication.to_string(), "medication");
        assert_eq!(ExtractionDomain::Appointment.to_string(), "appointment");
    }

    #[test]
    fn extraction_domain_from_invalid() {
        assert_eq!(ExtractionDomain::from_str("unknown"), None);
        assert_eq!(ExtractionDomain::from_str(""), None);
    }

    #[test]
    fn extraction_domain_all_has_three() {
        assert_eq!(ExtractionDomain::all().len(), 3);
    }

    #[test]
    fn grounding_roundtrip() {
        let variants = [Grounding::Grounded, Grounding::Partial, Grounding::Ungrounded];
        for g in &variants {
            let s = g.as_str();
            let parsed = Grounding::from_str(s);
            assert_eq!(parsed, Some(*g), "Roundtrip failed for {s}");
        }
    }

    #[test]
    fn grounding_display() {
        assert_eq!(Grounding::Grounded.to_string(), "grounded");
        assert_eq!(Grounding::Partial.to_string(), "partial");
        assert_eq!(Grounding::Ungrounded.to_string(), "ungrounded");
    }

    #[test]
    fn pending_status_roundtrip() {
        let variants = [
            PendingStatus::Pending,
            PendingStatus::Confirmed,
            PendingStatus::EditedConfirmed,
            PendingStatus::Dismissed,
        ];
        for s in &variants {
            let str_val = s.as_str();
            let parsed = PendingStatus::from_str(str_val);
            assert_eq!(parsed, Some(*s), "Roundtrip failed for {str_val}");
        }
    }

    #[test]
    fn batch_result_empty() {
        let result = BatchResult::empty();
        assert_eq!(result.conversations_processed, 0);
        assert_eq!(result.items_extracted, 0);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn extraction_config_defaults() {
        let config = ExtractionConfig::default();
        assert_eq!(config.batch_start_hour, Some(2));
        assert_eq!(config.batch_cooldown_hours, 6);
        assert_eq!(config.confidence_threshold, 0.7);
        assert_eq!(config.max_conversations_per_batch, 20);
        assert_eq!(config.max_items_per_domain, 5);
        assert_eq!(config.cold_hours, 6);
        assert_eq!(config.idle_minutes, 5);
    }

    #[test]
    fn extraction_domain_serde_roundtrip() {
        let domain = ExtractionDomain::Symptom;
        let json = serde_json::to_string(&domain).unwrap();
        assert_eq!(json, "\"symptom\"");
        let parsed: ExtractionDomain = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, domain);
    }

    #[test]
    fn grounding_serde_roundtrip() {
        let g = Grounding::Partial;
        let json = serde_json::to_string(&g).unwrap();
        assert_eq!(json, "\"partial\"");
        let parsed: Grounding = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, g);
    }

    #[test]
    fn pending_status_serde_roundtrip() {
        let s = PendingStatus::EditedConfirmed;
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"edited_confirmed\"");
        let parsed: PendingStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, s);
    }

    #[test]
    fn batch_status_event_serde() {
        let event = BatchStatusEvent::Progress {
            completed: 3,
            total: 7,
            current_title: "Headache discussion".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"Progress\""));
        assert!(json.contains("\"completed\":3"));
    }

    #[test]
    fn extracted_symptom_data_serde() {
        let data = ExtractedSymptomData {
            category: "Pain".to_string(),
            specific: "Headache".to_string(),
            severity_hint: Some(4),
            onset_hint: Some("2026-02-17".to_string()),
            body_region: Some("right side of head".to_string()),
            duration: Some("3 days".to_string()),
            character: Some("Throbbing".to_string()),
            aggravating: vec!["screen use".to_string()],
            relieving: vec![],
            timing_pattern: Some("Morning".to_string()),
            notes: None,
            related_medication_hint: Some("Lisinopril".to_string()),
            source_messages: vec![0, 1, 6],
        };
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("Headache"));
        assert!(json.contains("Throbbing"));
        // Verify null fields are skipped
        assert!(!json.contains("notes"));
    }

    #[test]
    fn extracted_medication_data_serde() {
        let data = ExtractedMedicationData {
            name: "Ibuprofen".to_string(),
            dose: Some("400mg".to_string()),
            frequency: Some("twice daily".to_string()),
            route: Some("oral".to_string()),
            reason: Some("headaches".to_string()),
            is_otc: true,
            start_date_hint: Some("2026-02-19".to_string()),
            status_hint: Some("active".to_string()),
            adherence_note: None,
            source_messages: vec![3],
        };
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("Ibuprofen"));
        assert!(json.contains("\"is_otc\":true"));
    }

    #[test]
    fn extracted_appointment_data_serde() {
        let data = ExtractedAppointmentData {
            professional_name: Some("Dr. Martin".to_string()),
            specialty: Some("Neurologist".to_string()),
            date_hint: Some("2026-02-25".to_string()),
            time_hint: Some("14:00".to_string()),
            location: None,
            reason: Some("headache consultation".to_string()),
            notes: None,
            source_messages: vec![4],
        };
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("Dr. Martin"));
        assert!(json.contains("Neurologist"));
    }

    #[test]
    fn pending_extraction_view_serde() {
        let view = PendingExtractionView {
            id: "test-id".to_string(),
            domain: ExtractionDomain::Symptom,
            confidence: 0.85,
            grounding: Grounding::Grounded,
            fields: serde_json::json!({"category": "Pain", "specific": "Headache"}),
            source_quote: "I've been having headaches for 3 days".to_string(),
            source_conversation_title: "Headache discussion".to_string(),
            duplicate_warning: None,
            created_at: "2026-02-20T08:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&view).unwrap();
        let parsed: PendingExtractionView = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "test-id");
        assert_eq!(parsed.domain, ExtractionDomain::Symptom);
        assert_eq!(parsed.confidence, 0.85);
    }

    #[test]
    fn dispatch_result_success() {
        let result = DispatchResult {
            item_id: "item-1".to_string(),
            domain: ExtractionDomain::Medication,
            success: true,
            created_record_id: Some("med-123".to_string()),
            error: None,
            correlations: None,
            duplicate_warning: None,
        };
        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn dispatch_result_failure() {
        let result = DispatchResult {
            item_id: "item-2".to_string(),
            domain: ExtractionDomain::Appointment,
            success: false,
            created_record_id: None,
            error: Some("Database constraint violation".to_string()),
            correlations: None,
            duplicate_warning: None,
        };
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn analysis_result_build_input_includes_surrounding_messages() {
        let conversation = ConversationBatch {
            id: "conv-1".to_string(),
            title: Some("Test".to_string()),
            messages: (0..5)
                .map(|i| ConversationMessage {
                    id: format!("msg-{i}"),
                    index: i,
                    role: if i % 2 == 0 { "patient".to_string() } else { "coheara".to_string() },
                    content: format!("Message {i}"),
                    created_at: chrono::NaiveDate::from_ymd_opt(2026, 2, 20)
                        .unwrap()
                        .and_hms_opt(10, 0, 0)
                        .unwrap(),
                    is_signal: false,
                })
                .collect(),
            last_message_at: chrono::NaiveDate::from_ymd_opt(2026, 2, 20)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
            message_count: 5,
        };

        let analysis = AnalysisResult {
            domains: vec![DomainMatch {
                domain: ExtractionDomain::Symptom,
                signal_message_indices: vec![2],
                detection_confidence: 0.9,
            }],
            is_pure_qa: false,
        };

        let input = analysis.build_input(
            &conversation,
            &analysis.domains[0],
            PatientContext::default(),
            chrono::NaiveDate::from_ymd_opt(2026, 2, 20).unwrap(),
        );

        // Signal at index 2 → should include 1 (before), 2 (signal), 3 (after)
        assert_eq!(input.messages.len(), 3);
        assert_eq!(input.messages[0].id, "msg-1");
        assert_eq!(input.messages[1].id, "msg-2");
        assert_eq!(input.messages[2].id, "msg-3");
    }

    #[test]
    fn analysis_result_build_input_handles_first_message() {
        let conversation = ConversationBatch {
            id: "conv-1".to_string(),
            title: None,
            messages: (0..3)
                .map(|i| ConversationMessage {
                    id: format!("msg-{i}"),
                    index: i,
                    role: "patient".to_string(),
                    content: format!("Message {i}"),
                    created_at: chrono::NaiveDate::from_ymd_opt(2026, 2, 20)
                        .unwrap()
                        .and_hms_opt(10, 0, 0)
                        .unwrap(),
                    is_signal: false,
                })
                .collect(),
            last_message_at: chrono::NaiveDate::from_ymd_opt(2026, 2, 20)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
            message_count: 3,
        };

        let analysis = AnalysisResult {
            domains: vec![DomainMatch {
                domain: ExtractionDomain::Symptom,
                signal_message_indices: vec![0],
                detection_confidence: 0.9,
            }],
            is_pure_qa: false,
        };

        let input = analysis.build_input(
            &conversation,
            &analysis.domains[0],
            PatientContext::default(),
            chrono::NaiveDate::from_ymd_opt(2026, 2, 20).unwrap(),
        );

        // Signal at index 0 → should include 0 (signal), 1 (after) only
        assert_eq!(input.messages.len(), 2);
        assert_eq!(input.messages[0].id, "msg-0");
        assert_eq!(input.messages[1].id, "msg-1");
    }

    #[test]
    fn analysis_result_build_input_handles_last_message() {
        let conversation = ConversationBatch {
            id: "conv-1".to_string(),
            title: None,
            messages: (0..3)
                .map(|i| ConversationMessage {
                    id: format!("msg-{i}"),
                    index: i,
                    role: "patient".to_string(),
                    content: format!("Message {i}"),
                    created_at: chrono::NaiveDate::from_ymd_opt(2026, 2, 20)
                        .unwrap()
                        .and_hms_opt(10, 0, 0)
                        .unwrap(),
                    is_signal: false,
                })
                .collect(),
            last_message_at: chrono::NaiveDate::from_ymd_opt(2026, 2, 20)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
            message_count: 3,
        };

        let analysis = AnalysisResult {
            domains: vec![DomainMatch {
                domain: ExtractionDomain::Symptom,
                signal_message_indices: vec![2],
                detection_confidence: 0.9,
            }],
            is_pure_qa: false,
        };

        let input = analysis.build_input(
            &conversation,
            &analysis.domains[0],
            PatientContext::default(),
            chrono::NaiveDate::from_ymd_opt(2026, 2, 20).unwrap(),
        );

        // Signal at index 2 (last) → should include 1 (before), 2 (signal) only
        assert_eq!(input.messages.len(), 2);
        assert_eq!(input.messages[0].id, "msg-1");
        assert_eq!(input.messages[1].id, "msg-2");
    }

    #[test]
    fn analysis_result_build_input_deduplicates_adjacent_signals() {
        let conversation = ConversationBatch {
            id: "conv-1".to_string(),
            title: None,
            messages: (0..5)
                .map(|i| ConversationMessage {
                    id: format!("msg-{i}"),
                    index: i,
                    role: "patient".to_string(),
                    content: format!("Message {i}"),
                    created_at: chrono::NaiveDate::from_ymd_opt(2026, 2, 20)
                        .unwrap()
                        .and_hms_opt(10, 0, 0)
                        .unwrap(),
                    is_signal: false,
                })
                .collect(),
            last_message_at: chrono::NaiveDate::from_ymd_opt(2026, 2, 20)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
            message_count: 5,
        };

        let analysis = AnalysisResult {
            domains: vec![DomainMatch {
                domain: ExtractionDomain::Symptom,
                // Two adjacent signal messages
                signal_message_indices: vec![1, 2],
                detection_confidence: 0.9,
            }],
            is_pure_qa: false,
        };

        let input = analysis.build_input(
            &conversation,
            &analysis.domains[0],
            PatientContext::default(),
            chrono::NaiveDate::from_ymd_opt(2026, 2, 20).unwrap(),
        );

        // Signals at 1, 2 → context: 0, 1, 2, 3 → deduplicated to 4 messages
        assert_eq!(input.messages.len(), 4);
        assert_eq!(input.messages[0].id, "msg-0");
        assert_eq!(input.messages[1].id, "msg-1");
        assert_eq!(input.messages[2].id, "msg-2");
        assert_eq!(input.messages[3].id, "msg-3");
    }
}
