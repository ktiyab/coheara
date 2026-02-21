//! M2: Conversation Analyzer — rule-based domain classification.
//!
//! Two-layer classification with no LLM calls:
//! - L1: Keyword scan (< 1ms) — fast lexical match
//! - L2: Pattern scan (< 5ms) — regex patterns for structured mentions
//!
//! With the full conversation available, keyword + pattern matching
//! achieves high enough recall for extraction eligibility.

use regex::Regex;
use std::sync::LazyLock;

use super::traits::ConversationAnalyzer;
use super::types::*;

// ═══════════════════════════════════════════
// Keyword Lists (L1)
// ═══════════════════════════════════════════

const SYMPTOM_KEYWORDS: &[&str] = &[
    // EN
    "pain", "headache", "nausea", "fatigue", "cough", "fever", "dizziness",
    "rash", "itch", "sore", "ache", "dizzy", "tired", "sleeping poorly",
    "vomiting", "diarrhea", "numbness", "tingling", "swelling", "bleeding",
    "shortness of breath", "chest pain", "back pain", "stomach",
    // FR
    "douleur", "mal de tête", "nausée", "fatigue", "toux", "fièvre",
    "vertige", "éruption", "démangeaison", "vomissement", "saignement",
    // DE
    "Kopfschmerzen", "Übelkeit", "Müdigkeit", "Husten", "Fieber",
    "Schwindel", "Ausschlag", "Schmerzen", "Durchfall",
];

const MEDICATION_KEYWORDS: &[&str] = &[
    // EN
    "medication", "medicine", "pill", "tablet", "capsule", "taking",
    "started", "stopped", "prescribed", "dose", "dosage",
    "ibuprofen", "paracetamol", "aspirin", "antibiotic",
    // FR
    "médicament", "comprimé", "posologie", "ordonnance", "gélule",
    // DE
    "Medikament", "Tablette", "Rezept", "Dosis",
];

const APPOINTMENT_KEYWORDS: &[&str] = &[
    // EN
    "appointment", "doctor", "visit", "specialist", "check-up",
    "neurologist", "cardiologist", "dermatologist", "surgeon",
    // FR
    "rendez-vous", "consultation", "médecin", "spécialiste",
    // DE
    "Termin", "Arzt", "Facharzt", "Untersuchung",
];

// ═══════════════════════════════════════════
// Pattern Lists (L2)
// ═══════════════════════════════════════════

static SYMPTOM_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)(?:i|I)\s+(?:have|had|feel|felt|notice|started|been having)\s+(?:a\s+|some\s+|bad\s+|terrible\s+)?\w+",
        r"(?i)(?:since|for)\s+(?:yesterday|this morning|last night|\d+\s+days?|\d+\s+weeks?)",
        r"(?i)(?:worse|better|improves|worsens)\s+(?:when|with|after|in the|at)",
        r"(?i)\d\s*/?\s*(?:out of\s+)?(?:10|5)\b",
        r"(?i)(?:throbbing|sharp|dull|burning|stabbing|aching|cramping)",
    ]
    .iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect()
});

static MEDICATION_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)(?:taking|started|stopped|prescribed|switched to)\s+\w+\s*\d+\s*mg",
        r"(?i)\d+\s*mg\s+(?:twice|once|three times|daily|per day|every)",
        r"(?i)(?:forgot|missed)\s+(?:my|the|a)\s+\w+",
        r"(?i)(?:mg|mcg|ml)\b",
    ]
    .iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect()
});

static APPOINTMENT_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)(?:appointment|visit|seeing)\s+(?:with|at|on)\s+",
        r"(?i)(?:next|this|coming)\s+(?:monday|tuesday|wednesday|thursday|friday|saturday|sunday)",
        r"(?i)(?:dr\.?|doctor|prof\.?)\s+[A-Z]\w+",
    ]
    .iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect()
});

// ═══════════════════════════════════════════
// Pure Q&A Detection
// ═══════════════════════════════════════════

/// Check if a conversation is purely informational (Q&A) with
/// no health data entry. Skip these to save LLM calls.
fn is_pure_qa(messages: &[ConversationMessage]) -> bool {
    let patient_messages: Vec<&ConversationMessage> = messages
        .iter()
        .filter(|m| m.role == "patient")
        .collect();

    if patient_messages.is_empty() {
        return true;
    }

    // All patient messages are questions (end with ?) and contain no health keywords
    let all_questions = patient_messages.iter().all(|m| {
        let trimmed = m.content.trim();
        trimmed.ends_with('?')
    });

    if !all_questions {
        return false;
    }

    // Check that no patient message contains any health domain keywords
    let has_health_keywords = patient_messages.iter().any(|m| {
        let lower = m.content.to_lowercase();
        SYMPTOM_KEYWORDS.iter().any(|k| lower.contains(&k.to_lowercase()))
            || MEDICATION_KEYWORDS.iter().any(|k| lower.contains(&k.to_lowercase()))
            || APPOINTMENT_KEYWORDS.iter().any(|k| lower.contains(&k.to_lowercase()))
    });

    !has_health_keywords
}

// ═══════════════════════════════════════════
// Implementation
// ═══════════════════════════════════════════

/// Rule-based conversation analyzer.
/// Uses L1 keyword + L2 pattern scanning to classify domains.
pub struct RuleBasedAnalyzer;

impl RuleBasedAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Scan a single message for keyword matches in a domain.
    fn keyword_match(content: &str, keywords: &[&str]) -> bool {
        let lower = content.to_lowercase();
        keywords.iter().any(|k| lower.contains(&k.to_lowercase()))
    }

    /// Scan a single message for pattern matches in a domain.
    fn pattern_match(content: &str, patterns: &[Regex]) -> bool {
        patterns.iter().any(|p| p.is_match(content))
    }

    /// Score a domain for a single message. Returns (keyword_hit, pattern_hit).
    fn score_message(
        content: &str,
        keywords: &[&str],
        patterns: &[Regex],
    ) -> (bool, bool) {
        let kw = Self::keyword_match(content, keywords);
        let pat = Self::pattern_match(content, patterns);
        (kw, pat)
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

        let mut symptom_signals = Vec::new();
        let mut medication_signals = Vec::new();
        let mut appointment_signals = Vec::new();

        // Only analyze patient messages for signals
        for msg in &conversation.messages {
            if msg.role != "patient" {
                continue;
            }

            let (s_kw, s_pat) = Self::score_message(
                &msg.content,
                SYMPTOM_KEYWORDS,
                &SYMPTOM_PATTERNS,
            );
            if s_kw || s_pat {
                symptom_signals.push(msg.index);
            }

            let (m_kw, m_pat) = Self::score_message(
                &msg.content,
                MEDICATION_KEYWORDS,
                &MEDICATION_PATTERNS,
            );
            if m_kw || m_pat {
                medication_signals.push(msg.index);
            }

            let (a_kw, a_pat) = Self::score_message(
                &msg.content,
                APPOINTMENT_KEYWORDS,
                &APPOINTMENT_PATTERNS,
            );
            if a_kw || a_pat {
                appointment_signals.push(msg.index);
            }
        }

        let mut domains = Vec::new();

        if !symptom_signals.is_empty() {
            // Confidence based on signal density
            let density = symptom_signals.len() as f32
                / conversation.messages.iter().filter(|m| m.role == "patient").count().max(1) as f32;
            domains.push(DomainMatch {
                domain: ExtractionDomain::Symptom,
                signal_message_indices: symptom_signals,
                detection_confidence: density.min(1.0),
            });
        }

        if !medication_signals.is_empty() {
            let density = medication_signals.len() as f32
                / conversation.messages.iter().filter(|m| m.role == "patient").count().max(1) as f32;
            domains.push(DomainMatch {
                domain: ExtractionDomain::Medication,
                signal_message_indices: medication_signals,
                detection_confidence: density.min(1.0),
            });
        }

        if !appointment_signals.is_empty() {
            let density = appointment_signals.len() as f32
                / conversation.messages.iter().filter(|m| m.role == "patient").count().max(1) as f32;
            domains.push(DomainMatch {
                domain: ExtractionDomain::Appointment,
                signal_message_indices: appointment_signals,
                detection_confidence: density.min(1.0),
            });
        }

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
    fn detects_symptom_keywords() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "I've been having headaches for 3 days"),
            make_message(1, "coheara", "I see, tell me more about the pain."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        assert!(!result.is_pure_qa);
        let symptom = result.domains.iter().find(|d| d.domain == ExtractionDomain::Symptom);
        assert!(symptom.is_some(), "Should detect symptom domain");
        assert!(symptom.unwrap().signal_message_indices.contains(&0));
    }

    #[test]
    fn detects_medication_keywords() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "I'm taking Lisinopril 10mg every morning"),
            make_message(1, "coheara", "That's a blood pressure medication."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        let medication = result.domains.iter().find(|d| d.domain == ExtractionDomain::Medication);
        assert!(medication.is_some(), "Should detect medication domain");
    }

    #[test]
    fn detects_appointment_keywords() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "I have an appointment with Dr. Martin next Tuesday"),
            make_message(1, "coheara", "Good, make sure to mention your symptoms."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        let appointment = result.domains.iter().find(|d| d.domain == ExtractionDomain::Appointment);
        assert!(appointment.is_some(), "Should detect appointment domain");
    }

    #[test]
    fn detects_multiple_domains() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "I've been having headaches for 3 days"),
            make_message(1, "coheara", "I see."),
            make_message(2, "patient", "I'm taking ibuprofen 400mg twice a day"),
            make_message(3, "coheara", "OK."),
            make_message(4, "patient", "I have an appointment with Dr. Martin"),
            make_message(5, "coheara", "Good."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        assert_eq!(result.domains.len(), 3, "Should detect all 3 domains");
    }

    #[test]
    fn pure_qa_conversation() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "What does a blood test measure?"),
            make_message(1, "coheara", "Blood tests can measure various things..."),
            make_message(2, "patient", "How often should I get one?"),
            make_message(3, "coheara", "It depends on your health conditions..."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        assert!(result.is_pure_qa, "Pure Q&A should be detected");
        assert!(result.domains.is_empty());
    }

    #[test]
    fn not_pure_qa_with_health_keywords_in_question() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "Does my headache mean something serious?"),
            make_message(1, "coheara", "Headaches can have many causes."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        // Even though it's a question, "headache" is a symptom keyword
        assert!(!result.is_pure_qa);
    }

    #[test]
    fn detects_symptom_patterns() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "The pain is throbbing, about 6/10"),
            make_message(1, "coheara", "That sounds uncomfortable."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        let symptom = result.domains.iter().find(|d| d.domain == ExtractionDomain::Symptom);
        assert!(symptom.is_some(), "Should detect symptom via pattern");
    }

    #[test]
    fn detects_medication_patterns() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "I started ibuprofen 400 mg since yesterday"),
            make_message(1, "coheara", "How is it helping?"),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        let medication = result.domains.iter().find(|d| d.domain == ExtractionDomain::Medication);
        assert!(medication.is_some(), "Should detect medication via pattern");
    }

    #[test]
    fn detects_appointment_patterns() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "I'm seeing Dr. Martin next Tuesday at 2pm"),
            make_message(1, "coheara", "Good to prepare."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        let appointment = result.domains.iter().find(|d| d.domain == ExtractionDomain::Appointment);
        assert!(appointment.is_some(), "Should detect appointment via pattern");
    }

    #[test]
    fn only_analyzes_patient_messages() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "Hello, how are you?"),
            make_message(1, "coheara", "I notice you mentioned headaches and pain and medication Ibuprofen 400mg"),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        // Assistant messages should not trigger detection
        assert!(result.domains.is_empty() || result.is_pure_qa);
    }

    #[test]
    fn french_symptom_detection() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "J'ai mal de tête depuis 3 jours"),
            make_message(1, "coheara", "Je comprends."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        let symptom = result.domains.iter().find(|d| d.domain == ExtractionDomain::Symptom);
        assert!(symptom.is_some(), "Should detect French symptom keyword");
    }

    #[test]
    fn german_medication_detection() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "Ich nehme mein Medikament jeden Morgen"),
            make_message(1, "coheara", "Gut."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        let medication = result.domains.iter().find(|d| d.domain == ExtractionDomain::Medication);
        assert!(medication.is_some(), "Should detect German medication keyword");
    }

    #[test]
    fn empty_conversation() {
        let conv = make_conversation(vec![]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        assert!(result.is_pure_qa);
        assert!(result.domains.is_empty());
    }

    #[test]
    fn confidence_scales_with_signal_density() {
        // 3 out of 3 patient messages mention symptoms → high confidence
        let conv_high = make_conversation(vec![
            make_message(0, "patient", "I have a headache"),
            make_message(1, "coheara", "Tell me more."),
            make_message(2, "patient", "Also nausea"),
            make_message(3, "coheara", "OK."),
            make_message(4, "patient", "And dizziness"),
            make_message(5, "coheara", "I see."),
        ]);

        // 1 out of 3 patient messages mentions symptoms → lower confidence
        let conv_low = make_conversation(vec![
            make_message(0, "patient", "I have a headache"),
            make_message(1, "coheara", "Tell me more."),
            make_message(2, "patient", "What should I eat for lunch?"),
            make_message(3, "coheara", "A balanced diet."),
            make_message(4, "patient", "Thanks for the info"),
            make_message(5, "coheara", "You're welcome."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();

        let high = analyzer.analyze(&conv_high);
        let low = analyzer.analyze(&conv_low);

        let high_conf = high.domains.iter()
            .find(|d| d.domain == ExtractionDomain::Symptom)
            .map(|d| d.detection_confidence)
            .unwrap_or(0.0);

        let low_conf = low.domains.iter()
            .find(|d| d.domain == ExtractionDomain::Symptom)
            .map(|d| d.detection_confidence)
            .unwrap_or(0.0);

        assert!(high_conf > low_conf, "Higher signal density should give higher confidence: {high_conf} > {low_conf}");
    }

    #[test]
    fn missed_dose_detected_as_medication() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "I forgot my Lisinopril yesterday morning"),
            make_message(1, "coheara", "Try to take it as soon as you remember."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        let medication = result.domains.iter().find(|d| d.domain == ExtractionDomain::Medication);
        assert!(medication.is_some(), "Missed dose should trigger medication detection");
    }

    #[test]
    fn benchmark_10_message_conversation() {
        // The MEDGEMMA-BENCHMARK-02 scenario
        let conv = make_conversation(vec![
            make_message(0, "patient", "I've been having headaches for the past 3 days. They're mostly in the morning when I wake up."),
            make_message(1, "coheara", "I understand you've been experiencing headaches."),
            make_message(2, "patient", "The pain is kind of throbbing, mostly on the right side of my head. I'd say it's about 6/10."),
            make_message(3, "coheara", "I understand the throbbing nature."),
            make_message(4, "patient", "I'm currently taking Lisinopril 10mg every morning for my blood pressure. Could that be related?"),
            make_message(5, "coheara", "It's possible that Lisinopril could be contributing."),
            make_message(6, "patient", "I also started taking ibuprofen 400mg twice a day since yesterday."),
            make_message(7, "coheara", "I understand you're taking ibuprofen."),
            make_message(8, "patient", "I have an appointment with Dr. Martin, my neurologist, next Tuesday at 2pm."),
            make_message(9, "coheara", "It's a good idea to be prepared."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        assert!(!result.is_pure_qa);
        assert_eq!(result.domains.len(), 3, "Should detect all 3 domains from benchmark conversation");

        // Verify specific signal messages
        let symptom = result.domains.iter().find(|d| d.domain == ExtractionDomain::Symptom).unwrap();
        assert!(symptom.signal_message_indices.contains(&0), "Message 0 should be symptom signal");
        assert!(symptom.signal_message_indices.contains(&2), "Message 2 should be symptom signal");

        let medication = result.domains.iter().find(|d| d.domain == ExtractionDomain::Medication).unwrap();
        assert!(medication.signal_message_indices.contains(&4), "Message 4 should be medication signal");
        assert!(medication.signal_message_indices.contains(&6), "Message 6 should be medication signal");

        let appointment = result.domains.iter().find(|d| d.domain == ExtractionDomain::Appointment).unwrap();
        assert!(appointment.signal_message_indices.contains(&8), "Message 8 should be appointment signal");
    }
}
