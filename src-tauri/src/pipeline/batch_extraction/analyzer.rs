//! M2: Conversation Analyzer — structural pre-filter.
//!
//! Skips pure Q&A conversations (all patient messages are questions).
//! For all other conversations, marks all domains as candidates and
//! delegates actual classification to the SLM extractor.

use super::traits::ConversationAnalyzer;
use super::types::*;

/// Check if a conversation is purely informational (Q&A) with
/// no health data entry. Skip these to save LLM calls.
///
/// A conversation is pure Q&A when all patient messages end with `?`.
fn is_pure_qa(messages: &[ConversationMessage]) -> bool {
    let patient_messages: Vec<&ConversationMessage> = messages
        .iter()
        .filter(|m| m.role == "patient")
        .collect();

    if patient_messages.is_empty() {
        return true;
    }

    // All patient messages are questions (end with ?)
    patient_messages.iter().all(|m| m.content.trim().ends_with('?'))
}

/// Structural conversation analyzer.
///
/// Uses only structural signals (message shape, not content keywords).
/// Actual domain classification is the SLM's responsibility.
pub struct RuleBasedAnalyzer;

impl RuleBasedAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl ConversationAnalyzer for RuleBasedAnalyzer {
    fn analyze(&self, conversation: &ConversationBatch) -> AnalysisResult {
        if is_pure_qa(&conversation.messages) {
            return AnalysisResult {
                domains: vec![],
                is_pure_qa: true,
            };
        }

        // Not pure Q&A — mark all domains as candidates.
        // The SLM extractor will determine which domains actually have data.
        let patient_indices: Vec<usize> = conversation
            .messages
            .iter()
            .filter(|m| m.role == "patient")
            .map(|m| m.index)
            .collect();

        let domains = vec![
            DomainMatch {
                domain: ExtractionDomain::Symptom,
                signal_message_indices: patient_indices.clone(),
                detection_confidence: 0.5,
            },
            DomainMatch {
                domain: ExtractionDomain::Medication,
                signal_message_indices: patient_indices.clone(),
                detection_confidence: 0.5,
            },
            DomainMatch {
                domain: ExtractionDomain::Appointment,
                signal_message_indices: patient_indices,
                detection_confidence: 0.5,
            },
        ];

        AnalysisResult {
            domains,
            is_pure_qa: false,
        }
    }
}

// ═══════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_message(index: usize, role: &str, content: &str) -> ConversationMessage {
        ConversationMessage {
            id: format!("msg-{index}"),
            index,
            role: role.to_string(),
            content: content.to_string(),
            created_at: chrono::NaiveDate::from_ymd_opt(2026, 2, 20)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
            is_signal: false,
        }
    }

    fn make_conversation(messages: Vec<ConversationMessage>) -> ConversationBatch {
        let count = messages.len() as u32;
        ConversationBatch {
            id: "conv-test".to_string(),
            title: Some("Test conversation".to_string()),
            messages,
            last_message_at: chrono::NaiveDate::from_ymd_opt(2026, 2, 20)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
            message_count: count,
        }
    }

    #[test]
    fn pure_qa_conversation_detected() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "What does a blood test measure?"),
            make_message(1, "coheara", "Blood tests can measure various things..."),
            make_message(2, "patient", "How often should I get one?"),
            make_message(3, "coheara", "It depends on your health conditions..."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        assert!(result.is_pure_qa);
        assert!(result.domains.is_empty());
    }

    #[test]
    fn non_question_triggers_all_domains() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "I've been having headaches for 3 days"),
            make_message(1, "coheara", "I see, tell me more."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        assert!(!result.is_pure_qa);
        assert_eq!(result.domains.len(), 3, "All 3 domains should be candidates");
    }

    #[test]
    fn mixed_questions_and_statements() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "I started taking ibuprofen"),
            make_message(1, "coheara", "OK."),
            make_message(2, "patient", "Is that safe?"),
            make_message(3, "coheara", "Generally yes."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        // Not pure Q&A because message 0 is a statement
        assert!(!result.is_pure_qa);
        assert_eq!(result.domains.len(), 3);
    }

    #[test]
    fn empty_conversation_is_pure_qa() {
        let conv = make_conversation(vec![]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        assert!(result.is_pure_qa);
        assert!(result.domains.is_empty());
    }

    #[test]
    fn only_assistant_messages_is_pure_qa() {
        let conv = make_conversation(vec![
            make_message(0, "coheara", "Hello, how can I help?"),
            make_message(1, "coheara", "Let me know if you have questions."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        assert!(result.is_pure_qa);
    }

    #[test]
    fn patient_indices_are_signal_messages() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "I feel tired"),
            make_message(1, "coheara", "Tell me more."),
            make_message(2, "patient", "Since yesterday"),
            make_message(3, "coheara", "I see."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        for domain in &result.domains {
            assert!(domain.signal_message_indices.contains(&0));
            assert!(domain.signal_message_indices.contains(&2));
            assert!(!domain.signal_message_indices.contains(&1));
            assert!(!domain.signal_message_indices.contains(&3));
        }
    }
}
