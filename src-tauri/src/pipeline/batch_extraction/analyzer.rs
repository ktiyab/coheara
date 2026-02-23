//! M2: Conversation Analyzer — L1+L2 classification pre-filter.
//!
//! Two-layer classification (no LLM needed):
//!   L1: Keyword scan (<1ms) — EN/FR/DE per domain
//!   L2: Regex pattern scan (<5ms) — structured patterns per domain
//!
//! Only domains with signals get extracted, saving 64% of LLM calls
//! per batch (MEDGEMMA-BENCHMARK-03 F1).

use std::sync::LazyLock;

use chrono::NaiveDate;
use regex::Regex;

use super::traits::ConversationAnalyzer;
use super::types::*;

// ═══════════════════════════════════════════
// L1: Keyword Lists (EN/FR/DE)
// ═══════════════════════════════════════════

const SYMPTOM_KEYWORDS: &[&str] = &[
    // EN
    "pain", "headache", "nausea", "fatigue", "cough", "fever", "dizziness",
    "rash", "itch", "sore", "ache", "dizzy", "tired", "insomnia",
    "vomiting", "swelling", "numbness", "tingling", "cramping",
    "bleeding", "shortness of breath", "chest pain", "anxiety",
    // FR
    "douleur", "mal de t\u{00ea}te", "naus\u{00e9}e", "fatigue", "toux", "fi\u{00e8}vre",
    "vertige", "\u{00e9}ruption", "d\u{00e9}mangeaison", "vomissement",
    "gonflement", "engourdissement", "crampe", "saignement",
    "essoufflement", "insomnie", "anxi\u{00e9}t\u{00e9}",
    // DE
    "Schmerz", "Kopfschmerzen", "\u{00dc}belkeit", "M\u{00fc}digkeit", "Husten", "Fieber",
    "Schwindel", "Ausschlag", "Juckreiz", "Erbrechen",
    "Schwellung", "Taubheit", "Krampf", "Blutung",
    "Atemnot", "Schlaflosigkeit",
];

const MEDICATION_KEYWORDS: &[&str] = &[
    // EN
    "medication", "medicine", "pill", "tablet", "capsule", "mg",
    "taking", "started", "stopped", "prescribed", "dose", "dosage",
    "ibuprofen", "paracetamol", "aspirin", "antibiotic",
    // FR
    "m\u{00e9}dicament", "comprim\u{00e9}", "posologie", "ordonnance",
    "g\u{00e9}lule", "sirop", "pommade",
    // DE
    "Medikament", "Tablette", "Rezept", "Kapsel",
    "Dosis", "Salbe",
];

const APPOINTMENT_KEYWORDS: &[&str] = &[
    // EN
    "appointment", "doctor", "visit", "specialist", "check-up",
    "checkup", "consultation", "clinic", "hospital", "dr.",
    // FR
    "rendez-vous", "consultation", "m\u{00e9}decin", "sp\u{00e9}cialiste",
    "h\u{00f4}pital", "clinique",
    // DE
    "Termin", "Arzt", "Facharzt", "Besuch",
    "Krankenhaus", "Klinik",
];

// ═══════════════════════════════════════════
// L2: Regex Patterns
// ═══════════════════════════════════════════

static SYMPTOM_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)(?:i|I)\s+(?:have|had|feel|felt|notice|noticed|started)\s+(?:a\s+|some\s+|bad\s+|terrible\s+)?(?:\w+\s+)?(?:pain|ache|headache|nausea|fatigue|dizziness)",
        r"(?i)(?:since|for)\s+(?:yesterday|this morning|last night|last week|\d+\s+(?:days?|weeks?|hours?|months?))",
        r"(?i)(?:worse|better|worsens?|improves?)\s+(?:when|with|after|in the|at|during)",
        r"(?i)\d\s*/?\s*(?:out of\s+)?(?:10|5)\b",
    ]
    .iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect()
});

static MEDICATION_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)(?:taking|started|stopped|prescribed|switched)\s+\w+\s*\d+\s*mg",
        r"(?i)\d+\s*mg\s+(?:twice|once|three times|daily|per day|a day)",
        r"(?i)(?:forgot|missed|skipped)\s+(?:my|the|a)\s+\w+",
    ]
    .iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect()
});

static APPOINTMENT_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)(?:appointment|visit|seeing)\s+(?:with|at|on|for)\s+",
        r"(?i)(?:next|this|coming)\s+(?:monday|tuesday|wednesday|thursday|friday|saturday|sunday|week|month)",
        r"(?i)(?:dr\.?|doctor|prof\.?)\s+[A-Z\u{00C0}-\u{00FF}]\w+",
    ]
    .iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect()
});

// ═══════════════════════════════════════════
// Pure Q&A Detection
// ═══════════════════════════════════════════

/// A conversation is pure Q&A when all patient messages end with `?`.
fn is_pure_qa(messages: &[ConversationMessage]) -> bool {
    let patient_messages: Vec<&ConversationMessage> = messages
        .iter()
        .filter(|m| m.role == "patient")
        .collect();

    if patient_messages.is_empty() {
        return true;
    }

    patient_messages.iter().all(|m| m.content.trim().ends_with('?'))
}

// ═══════════════════════════════════════════
// L1+L2 Classification
// ═══════════════════════════════════════════

/// Check if text contains any keyword from the list (case-insensitive).
fn has_keyword(text: &str, keywords: &[&str]) -> bool {
    let lower = text.to_lowercase();
    keywords.iter().any(|kw| lower.contains(&kw.to_lowercase()))
}

/// Check if text matches any pattern from the list.
fn has_pattern(text: &str, patterns: &[Regex]) -> bool {
    patterns.iter().any(|p| p.is_match(text))
}

/// Classify a single patient message against all domains.
/// Returns a bitmask-like tuple: (symptom, medication, appointment).
fn classify_message(content: &str) -> (bool, bool, bool) {
    let symptom = has_keyword(content, SYMPTOM_KEYWORDS) || has_pattern(content, &SYMPTOM_PATTERNS);
    let medication = has_keyword(content, MEDICATION_KEYWORDS) || has_pattern(content, &MEDICATION_PATTERNS);
    let appointment = has_keyword(content, APPOINTMENT_KEYWORDS) || has_pattern(content, &APPOINTMENT_PATTERNS);
    (symptom, medication, appointment)
}

// ═══════════════════════════════════════════
// Analyzer Implementation
// ═══════════════════════════════════════════

/// L1+L2 conversation analyzer.
///
/// Uses keyword scan (L1) + regex pattern scan (L2) to classify
/// patient messages into domains. Only domains with signals trigger
/// LLM extraction calls.
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

        let mut symptom_signals = Vec::new();
        let mut medication_signals = Vec::new();
        let mut appointment_signals = Vec::new();

        for msg in &conversation.messages {
            if msg.role != "patient" {
                continue;
            }

            let (is_symptom, is_medication, is_appointment) = classify_message(&msg.content);

            if is_symptom {
                symptom_signals.push(msg.index);
            }
            if is_medication {
                medication_signals.push(msg.index);
            }
            if is_appointment {
                appointment_signals.push(msg.index);
            }
        }

        let mut domains = Vec::new();

        if !symptom_signals.is_empty() {
            domains.push(DomainMatch {
                domain: ExtractionDomain::Symptom,
                signal_message_indices: symptom_signals,
                detection_confidence: 0.8,
            });
        }

        if !medication_signals.is_empty() {
            domains.push(DomainMatch {
                domain: ExtractionDomain::Medication,
                signal_message_indices: medication_signals,
                detection_confidence: 0.8,
            });
        }

        if !appointment_signals.is_empty() {
            domains.push(DomainMatch {
                domain: ExtractionDomain::Appointment,
                signal_message_indices: appointment_signals,
                detection_confidence: 0.8,
            });
        }

        AnalysisResult {
            domains,
            is_pure_qa: false,
        }
    }
}

// ═══════════════════════════════════════════
// Date Normalization Helper
// ═══════════════════════════════════════════

/// Resolve a relative date hint to an absolute date.
///
/// Handles EN/FR/DE relative references (MEDGEMMA-BENCHMARK-03 F2):
/// - "yesterday" / "hier" / "gestern"
/// - "N days ago" / "il y a N jours" / "vor N Tagen"
/// - "last week" / "la semaine derniere" / "letzte Woche"
/// - Day-of-week references: "next Tuesday" / "mardi prochain" / "n\u{00e4}chsten Dienstag"
///
/// Returns None if the hint can't be resolved.
pub fn normalize_date_hint(hint: &str, anchor: NaiveDate) -> Option<NaiveDate> {
    let lower = hint.trim().to_lowercase();

    // "yesterday" / "hier" / "gestern"
    if lower == "yesterday" || lower == "hier" || lower == "gestern" {
        return anchor.pred_opt();
    }

    // "today" / "aujourd'hui" / "heute"
    if lower == "today" || lower == "aujourd'hui" || lower == "heute" {
        return Some(anchor);
    }

    // "N days ago" / "il y a N jours" / "vor N Tagen"
    if let Some(days) = parse_days_ago(&lower) {
        return anchor.checked_sub_signed(chrono::Duration::days(days));
    }

    // "last week" / "la semaine derniere" / "letzte Woche"
    if lower == "last week"
        || lower == "la semaine derni\u{00e8}re"
        || lower == "la semaine derniere"
        || lower == "letzte woche"
    {
        return anchor.checked_sub_signed(chrono::Duration::days(7));
    }

    // "next <weekday>" / "<weekday> prochain" / "n\u{00e4}chsten <weekday>"
    if let Some(date) = parse_next_weekday(&lower, anchor) {
        return Some(date);
    }

    None
}

/// Parse "N days ago", "il y a N jours", "vor N Tagen" patterns.
fn parse_days_ago(lower: &str) -> Option<i64> {
    // EN: "3 days ago", "1 day ago"
    static RE_EN: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(\d+)\s+days?\s+ago").unwrap()
    });
    if let Some(caps) = RE_EN.captures(lower) {
        return caps.get(1).and_then(|m| m.as_str().parse().ok());
    }

    // FR: "il y a 3 jours"
    static RE_FR: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"il y a\s+(\d+)\s+jours?").unwrap()
    });
    if let Some(caps) = RE_FR.captures(lower) {
        return caps.get(1).and_then(|m| m.as_str().parse().ok());
    }

    // DE: "vor 3 Tagen"
    static RE_DE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"vor\s+(\d+)\s+tagen?").unwrap()
    });
    if let Some(caps) = RE_DE.captures(lower) {
        return caps.get(1).and_then(|m| m.as_str().parse().ok());
    }

    None
}

/// Parse "next Tuesday" / "mardi prochain" / "n\u{00e4}chsten Dienstag".
fn parse_next_weekday(lower: &str, anchor: NaiveDate) -> Option<NaiveDate> {
    use chrono::Datelike;

    let weekday_map: &[(&str, chrono::Weekday)] = &[
        // EN
        ("monday", chrono::Weekday::Mon),
        ("tuesday", chrono::Weekday::Tue),
        ("wednesday", chrono::Weekday::Wed),
        ("thursday", chrono::Weekday::Thu),
        ("friday", chrono::Weekday::Fri),
        ("saturday", chrono::Weekday::Sat),
        ("sunday", chrono::Weekday::Sun),
        // FR
        ("lundi", chrono::Weekday::Mon),
        ("mardi", chrono::Weekday::Tue),
        ("mercredi", chrono::Weekday::Wed),
        ("jeudi", chrono::Weekday::Thu),
        ("vendredi", chrono::Weekday::Fri),
        ("samedi", chrono::Weekday::Sat),
        ("dimanche", chrono::Weekday::Sun),
        // DE
        ("montag", chrono::Weekday::Mon),
        ("dienstag", chrono::Weekday::Tue),
        ("mittwoch", chrono::Weekday::Wed),
        ("donnerstag", chrono::Weekday::Thu),
        ("freitag", chrono::Weekday::Fri),
        ("samstag", chrono::Weekday::Sat),
        ("sonntag", chrono::Weekday::Sun),
    ];

    // Check for "next <day>" or "<day> prochain" or "n\u{00e4}chsten <day>"
    let is_next = lower.contains("next ")
        || lower.contains(" prochain")
        || lower.contains("n\u{00e4}chsten ")
        || lower.contains("nachsten ");

    if !is_next {
        return None;
    }

    for (name, weekday) in weekday_map {
        if lower.contains(name) {
            // Find the next occurrence of this weekday after anchor
            let current_weekday = anchor.weekday().num_days_from_monday();
            let target_weekday = weekday.num_days_from_monday();
            let days_ahead = if target_weekday > current_weekday {
                target_weekday - current_weekday
            } else {
                7 - (current_weekday - target_weekday)
            };
            return anchor.checked_add_signed(chrono::Duration::days(days_ahead as i64));
        }
    }

    None
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

    // ── Pure Q&A detection (preserved from existing) ──

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

    // ── L1+L2 domain classification ──

    #[test]
    fn symptom_only_triggers_symptom_domain() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "I've been having headaches for 3 days"),
            make_message(1, "coheara", "Tell me more."),
            make_message(2, "patient", "The pain is throbbing on the right side"),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        assert!(!result.is_pure_qa);
        assert_eq!(result.domains.len(), 1, "Only symptom domain should trigger");
        assert_eq!(result.domains[0].domain, ExtractionDomain::Symptom);
        assert!(result.domains[0].signal_message_indices.contains(&0));
        assert!(result.domains[0].signal_message_indices.contains(&2));
    }

    #[test]
    fn medication_only_triggers_medication_domain() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "I started taking ibuprofen 400mg twice daily"),
            make_message(1, "coheara", "OK, noted."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        assert!(!result.is_pure_qa);
        let med_domains: Vec<_> = result.domains.iter()
            .filter(|d| d.domain == ExtractionDomain::Medication)
            .collect();
        assert_eq!(med_domains.len(), 1, "Medication domain should trigger");
        assert!(med_domains[0].signal_message_indices.contains(&0));
    }

    #[test]
    fn appointment_only_triggers_appointment_domain() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "I have an appointment with Dr. Martin next Tuesday"),
            make_message(1, "coheara", "I'll note that."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        assert!(!result.is_pure_qa);
        let apt_domains: Vec<_> = result.domains.iter()
            .filter(|d| d.domain == ExtractionDomain::Appointment)
            .collect();
        assert_eq!(apt_domains.len(), 1, "Appointment domain should trigger");
        assert!(apt_domains[0].signal_message_indices.contains(&0));
    }

    #[test]
    fn multi_domain_conversation_triggers_multiple_domains() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "I've been having headaches"),
            make_message(1, "coheara", "Tell me more."),
            make_message(2, "patient", "I started taking ibuprofen 400mg"),
            make_message(3, "coheara", "Noted."),
            make_message(4, "patient", "I have a doctor appointment next week"),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        assert!(!result.is_pure_qa);
        let domains: Vec<ExtractionDomain> = result.domains.iter().map(|d| d.domain).collect();
        assert!(domains.contains(&ExtractionDomain::Symptom));
        assert!(domains.contains(&ExtractionDomain::Medication));
        assert!(domains.contains(&ExtractionDomain::Appointment));
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
        // Medication domain should trigger from msg 0 keyword "ibuprofen" + "taking"
        let med_domains: Vec<_> = result.domains.iter()
            .filter(|d| d.domain == ExtractionDomain::Medication)
            .collect();
        assert!(!med_domains.is_empty(), "Medication domain should be detected");
    }

    #[test]
    fn no_domain_signals_returns_empty() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "Thank you for the information"),
            make_message(1, "coheara", "You're welcome!"),
            make_message(2, "patient", "That's very helpful, goodbye"),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        assert!(!result.is_pure_qa); // Not pure Q&A (no question marks)
        assert!(result.domains.is_empty(), "No domain signals should be found");
    }

    // ── French keyword detection ──

    #[test]
    fn french_symptom_keywords_detected() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "J'ai des maux de t\u{00ea}te depuis 3 jours"),
            make_message(1, "coheara", "Pouvez-vous d\u{00e9}crire?"),
            make_message(2, "patient", "C'est une douleur pulsatile"),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        assert!(!result.is_pure_qa);
        let symptom_domains: Vec<_> = result.domains.iter()
            .filter(|d| d.domain == ExtractionDomain::Symptom)
            .collect();
        assert!(!symptom_domains.is_empty(), "French symptom keywords should be detected");
    }

    #[test]
    fn french_medication_keywords_detected() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "Je prends du m\u{00e9}dicament pour la douleur"),
            make_message(1, "coheara", "Lequel?"),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        let med_domains: Vec<_> = result.domains.iter()
            .filter(|d| d.domain == ExtractionDomain::Medication)
            .collect();
        assert!(!med_domains.is_empty(), "French medication keywords should be detected");
    }

    #[test]
    fn french_appointment_keywords_detected() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "J'ai un rendez-vous avec le m\u{00e9}decin"),
            make_message(1, "coheara", "Quand?"),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        let apt_domains: Vec<_> = result.domains.iter()
            .filter(|d| d.domain == ExtractionDomain::Appointment)
            .collect();
        assert!(!apt_domains.is_empty(), "French appointment keywords should be detected");
    }

    // ── German keyword detection ──

    #[test]
    fn german_symptom_keywords_detected() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "Ich habe seit 3 Tagen Kopfschmerzen"),
            make_message(1, "coheara", "Erz\u{00e4}hlen Sie mehr."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        let symptom_domains: Vec<_> = result.domains.iter()
            .filter(|d| d.domain == ExtractionDomain::Symptom)
            .collect();
        assert!(!symptom_domains.is_empty(), "German symptom keywords should be detected");
    }

    #[test]
    fn german_appointment_keywords_detected() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "Ich habe einen Termin beim Arzt"),
            make_message(1, "coheara", "Wann?"),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        let apt_domains: Vec<_> = result.domains.iter()
            .filter(|d| d.domain == ExtractionDomain::Appointment)
            .collect();
        assert!(!apt_domains.is_empty(), "German appointment keywords should be detected");
    }

    // ── L2 Pattern detection ──

    #[test]
    fn severity_pattern_detected() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "It's about 6 out of 10"),
            make_message(1, "coheara", "I see."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        let symptom_domains: Vec<_> = result.domains.iter()
            .filter(|d| d.domain == ExtractionDomain::Symptom)
            .collect();
        assert!(!symptom_domains.is_empty(), "Severity pattern should trigger symptom domain");
    }

    #[test]
    fn medication_dosage_pattern_detected() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "I'm taking metformin 500 mg twice daily"),
            make_message(1, "coheara", "Noted."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        let med_domains: Vec<_> = result.domains.iter()
            .filter(|d| d.domain == ExtractionDomain::Medication)
            .collect();
        assert!(!med_domains.is_empty(), "Medication dosage pattern should trigger");
    }

    #[test]
    fn doctor_name_pattern_detected() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "I'm seeing Dr. Martin on Friday"),
            make_message(1, "coheara", "Noted."),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        let apt_domains: Vec<_> = result.domains.iter()
            .filter(|d| d.domain == ExtractionDomain::Appointment)
            .collect();
        assert!(!apt_domains.is_empty(), "Doctor name pattern should trigger appointment domain");
    }

    // ── Signal message specificity ──

    #[test]
    fn signal_messages_are_specific_to_domain() {
        let conv = make_conversation(vec![
            make_message(0, "patient", "I've been having terrible headaches"),
            make_message(1, "coheara", "I see."),
            make_message(2, "patient", "I started taking ibuprofen 400mg"),
            make_message(3, "coheara", "How often?"),
            make_message(4, "patient", "Twice a day, and I see Dr. Martin next Tuesday"),
        ]);

        let analyzer = RuleBasedAnalyzer::new();
        let result = analyzer.analyze(&conv);

        let symptom = result.domains.iter().find(|d| d.domain == ExtractionDomain::Symptom);
        let medication = result.domains.iter().find(|d| d.domain == ExtractionDomain::Medication);
        let appointment = result.domains.iter().find(|d| d.domain == ExtractionDomain::Appointment);

        assert!(symptom.is_some(), "Symptom domain should be present");
        assert!(medication.is_some(), "Medication domain should be present");
        assert!(appointment.is_some(), "Appointment domain should be present");

        // Msg 0 should only be symptom, not medication or appointment
        let sym = symptom.unwrap();
        assert!(sym.signal_message_indices.contains(&0));
    }

    // ── Date normalization ──

    #[test]
    fn normalize_yesterday_en() {
        let anchor = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        assert_eq!(
            normalize_date_hint("yesterday", anchor),
            Some(NaiveDate::from_ymd_opt(2026, 2, 19).unwrap())
        );
    }

    #[test]
    fn normalize_yesterday_fr() {
        let anchor = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        assert_eq!(
            normalize_date_hint("hier", anchor),
            Some(NaiveDate::from_ymd_opt(2026, 2, 19).unwrap())
        );
    }

    #[test]
    fn normalize_yesterday_de() {
        let anchor = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        assert_eq!(
            normalize_date_hint("gestern", anchor),
            Some(NaiveDate::from_ymd_opt(2026, 2, 19).unwrap())
        );
    }

    #[test]
    fn normalize_days_ago_en() {
        let anchor = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        assert_eq!(
            normalize_date_hint("3 days ago", anchor),
            Some(NaiveDate::from_ymd_opt(2026, 2, 17).unwrap())
        );
    }

    #[test]
    fn normalize_days_ago_fr() {
        let anchor = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        assert_eq!(
            normalize_date_hint("il y a 3 jours", anchor),
            Some(NaiveDate::from_ymd_opt(2026, 2, 17).unwrap())
        );
    }

    #[test]
    fn normalize_days_ago_de() {
        let anchor = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        assert_eq!(
            normalize_date_hint("vor 3 Tagen", anchor),
            Some(NaiveDate::from_ymd_opt(2026, 2, 17).unwrap())
        );
    }

    #[test]
    fn normalize_next_tuesday() {
        // 2026-02-20 is a Friday. Next Tuesday = 2026-02-24.
        let anchor = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        let result = normalize_date_hint("next Tuesday", anchor);
        // Tuesday = weekday 1 (Mon=0). Friday = weekday 4.
        // days_ahead = 7 - (4 - 1) = 4. 2026-02-20 + 4 = 2026-02-24.
        assert_eq!(
            result,
            Some(NaiveDate::from_ymd_opt(2026, 2, 24).unwrap())
        );
    }

    #[test]
    fn normalize_mardi_prochain() {
        let anchor = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        let result = normalize_date_hint("mardi prochain", anchor);
        assert_eq!(
            result,
            Some(NaiveDate::from_ymd_opt(2026, 2, 24).unwrap())
        );
    }

    #[test]
    fn normalize_unresolvable_returns_none() {
        let anchor = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        assert_eq!(normalize_date_hint("sometime soon", anchor), None);
    }

    #[test]
    fn normalize_today() {
        let anchor = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        assert_eq!(
            normalize_date_hint("today", anchor),
            Some(anchor)
        );
    }

    #[test]
    fn normalize_last_week() {
        let anchor = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();
        assert_eq!(
            normalize_date_hint("last week", anchor),
            Some(NaiveDate::from_ymd_opt(2026, 2, 13).unwrap())
        );
    }
}
