use std::sync::LazyLock;

use regex::Regex;

use super::types::{Violation, ViolationCategory};

/// Rephrase rules: deterministic transformations applied per violation category.
/// Each rule maps a violation pattern to a safe replacement pattern.
struct RephraseRule {
    /// Pattern to find the violating text.
    pattern: Regex,
    /// Replacement template ($1, $2 for capture groups).
    replacement: &'static str,
    /// Which violation category this rule addresses.
    category: ViolationCategory,
}

/// All rephrase rules per spec — 19 rules with capture groups.
static REPHRASE_RULES: LazyLock<Vec<RephraseRule>> = LazyLock::new(|| {
    vec![
        // --- Diagnostic → Document-attributed ---
        RephraseRule {
            pattern: Regex::new(r"(?i)\byou\s+have\s+((?:a\s+)?[\w\s]+?)([.!?,])").unwrap(),
            replacement: "your documents mention $1$2",
            category: ViolationCategory::DiagnosticLanguage,
        },
        RephraseRule {
            pattern: Regex::new(r"(?i)\byou\s+are\s+suffering\s+from\s+([\w\s]+?)([.!?,])").unwrap(),
            replacement: "your records reference $1$2",
            category: ViolationCategory::DiagnosticLanguage,
        },
        RephraseRule {
            pattern: Regex::new(r"(?i)\byou\s+(?:likely|probably|possibly)\s+have\s+([\w\s]+?)([.!?,])").unwrap(),
            replacement: "your documents may suggest $1$2",
            category: ViolationCategory::DiagnosticLanguage,
        },
        RephraseRule {
            pattern: Regex::new(r"(?i)\byou(?:'re|\s+are)\s+(?:a\s+)?(diabetic|hypertensive|anemic|asthmatic)\b").unwrap(),
            replacement: "your records indicate a diagnosis related to being $1",
            category: ViolationCategory::DiagnosticLanguage,
        },
        RephraseRule {
            pattern: Regex::new(r"(?i)\byou\s+(?:appear|seem)\s+to\s+have\s+([\w\s]+?)([.!?,])").unwrap(),
            replacement: "your documents reference $1$2",
            category: ViolationCategory::DiagnosticLanguage,
        },

        // --- Prescriptive → Suggestion to discuss ---
        RephraseRule {
            pattern: Regex::new(r"(?i)\byou\s+should\s+(take|stop|start|increase|decrease|change|switch|discontinue|avoid|reduce)\s+([\w\s]+?)([.!?,])").unwrap(),
            replacement: "you might want to discuss with your doctor whether to $1 $2$3",
            category: ViolationCategory::PrescriptiveLanguage,
        },
        RephraseRule {
            pattern: Regex::new(r"(?i)\bI\s+recommend\s+([\w\s]+?)([.!?,])").unwrap(),
            replacement: "you may want to ask your healthcare provider about $1$2",
            category: ViolationCategory::PrescriptiveLanguage,
        },
        RephraseRule {
            pattern: Regex::new(r"(?i)\bI\s+(?:would\s+)?(?:suggest|advise)\s+([\w\s]+?)([.!?,])").unwrap(),
            replacement: "it might be worth discussing with your doctor $1$2",
            category: ViolationCategory::PrescriptiveLanguage,
        },
        RephraseRule {
            pattern: Regex::new(r"(?i)\byou\s+(?:need|must|have)\s+to\s+(take|stop|start|see|visit|go|call|increase|decrease)\s+([\w\s]+?)([.!?,])").unwrap(),
            replacement: "you may want to talk with your healthcare provider about whether to $1 $2$3",
            category: ViolationCategory::PrescriptiveLanguage,
        },
        RephraseRule {
            pattern: Regex::new(r"(?i)\bdo\s+not\s+(take|stop|eat|drink|use|skip)\s+([\w\s]+?)([.!?,])").unwrap(),
            replacement: "you might want to ask your doctor before deciding to $1 $2$3",
            category: ViolationCategory::PrescriptiveLanguage,
        },

        // --- Alarm → Calm preparatory framing (NC-07) ---
        RephraseRule {
            pattern: Regex::new(r"(?i)\b(?:immediately|urgently)\s+(go|call|visit|see|seek|get)\b").unwrap(),
            replacement: "it may be helpful to $1",
            category: ViolationCategory::AlarmLanguage,
        },
        RephraseRule {
            pattern: Regex::new(r"(?i)\bthis\s+(?:is|could\s+be)\s+(?:a\s+)?(?:medical\s+)?emergency\b").unwrap(),
            replacement: "this is something you may want to discuss with your healthcare provider soon",
            category: ViolationCategory::AlarmLanguage,
        },
        RephraseRule {
            pattern: Regex::new(r"(?i)\bseek\s+(?:immediate|emergency|urgent)\s+(?:medical\s+)?(?:help|attention|care)\b").unwrap(),
            replacement: "consider reaching out to your healthcare provider",
            category: ViolationCategory::AlarmLanguage,
        },
        RephraseRule {
            pattern: Regex::new(r"(?i)\bcall\s+(?:911|emergency|an\s+ambulance)\b").unwrap(),
            replacement: "consider contacting your healthcare provider",
            category: ViolationCategory::AlarmLanguage,
        },
        RephraseRule {
            pattern: Regex::new(r"(?i)\bgo\s+to\s+(?:the\s+)?(?:emergency|ER|hospital|A&E)\b").unwrap(),
            replacement: "consider visiting your healthcare provider",
            category: ViolationCategory::AlarmLanguage,
        },
        RephraseRule {
            pattern: Regex::new(r"(?i)\bdangerous\b").unwrap(),
            replacement: "notable",
            category: ViolationCategory::AlarmLanguage,
        },
        RephraseRule {
            pattern: Regex::new(r"(?i)\b(?:life[- ]threatening|fatal|deadly|lethal)\b").unwrap(),
            replacement: "significant",
            category: ViolationCategory::AlarmLanguage,
        },
        RephraseRule {
            pattern: Regex::new(r"(?i)\bdo\s+not\s+(?:wait|delay|ignore)\b").unwrap(),
            replacement: "it may be worth bringing this up",
            category: ViolationCategory::AlarmLanguage,
        },

        // --- Ungrounded → Document-attributed ---
        RephraseRule {
            pattern: Regex::new(r"(?i)\byour\s+(blood\s+pressure|cholesterol|glucose|sugar|levels?|count|heart\s+rate|weight|BMI)\s+(is|are)\s+(high|low|elevated|abnormal|concerning|worrying|critical)\b").unwrap(),
            replacement: "your documents note that your $1 $2 $3",
            category: ViolationCategory::UngroundedClaim,
        },
    ]
});

/// Attempt to rephrase violations out of the text.
/// Returns `Some(rephrased)` if at least one rule applied, `None` if no rules could fix it.
pub fn rephrase_violations(text: &str, violations: &[Violation]) -> Option<String> {
    if violations.is_empty() {
        return Some(text.to_string());
    }

    let mut result = text.to_string();
    let mut applied_count = 0;

    // Sort violations by offset descending so replacements don't shift positions
    let mut sorted_violations = violations.to_vec();
    sorted_violations.sort_by(|a, b| b.offset.cmp(&a.offset));

    for violation in &sorted_violations {
        // Find matching rephrase rules for this violation category
        let applicable_rules: Vec<&RephraseRule> = REPHRASE_RULES
            .iter()
            .filter(|r| r.category == violation.category)
            .collect();

        for rule in &applicable_rules {
            let before = result.clone();
            result = rule.pattern.replace_all(&result, rule.replacement).to_string();
            if result != before {
                applied_count += 1;
                break; // One rule applied per violation
            }
        }
    }

    if applied_count == 0 {
        // No rules could be applied — rephrasing failed
        return None;
    }

    Some(result)
}

/// Select an appropriate fallback message based on violation types.
/// Returns English fallback — use `select_fallback_message_i18n` for localized messages.
pub fn select_fallback_message(violations: &[Violation]) -> String {
    select_fallback_message_i18n(violations, "en")
}

/// I18N-06: Select an appropriate fallback message in the given language.
/// Falls back to English for unsupported languages.
pub fn select_fallback_message_i18n(violations: &[Violation], lang: &str) -> String {
    // Prioritize by severity
    let has_alarm = violations
        .iter()
        .any(|v| v.category == ViolationCategory::AlarmLanguage);
    let has_prescriptive = violations
        .iter()
        .any(|v| v.category == ViolationCategory::PrescriptiveLanguage);
    let has_diagnostic = violations
        .iter()
        .any(|v| v.category == ViolationCategory::DiagnosticLanguage);

    match lang {
        "fr" => select_fallback_fr(has_alarm, has_prescriptive, has_diagnostic),
        "de" => select_fallback_de(has_alarm, has_prescriptive, has_diagnostic),
        _ => select_fallback_en(has_alarm, has_prescriptive, has_diagnostic),
    }
}

fn select_fallback_en(has_alarm: bool, has_prescriptive: bool, has_diagnostic: bool) -> String {
    if has_alarm {
        "I can help you understand what your medical documents say. \
         If you have health concerns, your healthcare provider is the best person to talk to."
            .to_string()
    } else if has_prescriptive {
        "I can help you understand your documents, but I'm not able to recommend \
         treatments or actions. Your healthcare provider can help with that. \
         Would you like me to help you prepare a question for your next appointment?"
            .to_string()
    } else if has_diagnostic {
        "I can share what your documents say, but I'm not able to make diagnoses. \
         Would you like me to explain what your documents mention?"
            .to_string()
    } else {
        "I can help you understand your medical documents. \
         Could you rephrase your question about your documents?"
            .to_string()
    }
}

/// I18N-06: French fallback messages — formal "vous" address.
fn select_fallback_fr(has_alarm: bool, has_prescriptive: bool, has_diagnostic: bool) -> String {
    if has_alarm {
        "Je peux vous aider à comprendre vos documents médicaux. \
         Si vous avez des préoccupations de santé, votre professionnel de santé est la personne la mieux placée pour en parler."
            .to_string()
    } else if has_prescriptive {
        "Je peux vous aider à comprendre vos documents, mais je ne suis pas en mesure de recommander \
         des traitements ou des actions. Votre professionnel de santé peut vous aider. \
         Souhaitez-vous que je vous aide à préparer une question pour votre prochain rendez-vous ?"
            .to_string()
    } else if has_diagnostic {
        "Je peux partager ce que vos documents indiquent, mais je ne suis pas en mesure de poser des diagnostics. \
         Souhaitez-vous que je vous explique ce que vos documents mentionnent ?"
            .to_string()
    } else {
        "Je peux vous aider à comprendre vos documents médicaux. \
         Pourriez-vous reformuler votre question concernant vos documents ?"
            .to_string()
    }
}

/// I18N-06: German fallback messages — formal "Sie" address.
fn select_fallback_de(has_alarm: bool, has_prescriptive: bool, has_diagnostic: bool) -> String {
    if has_alarm {
        "Ich kann Ihnen helfen, Ihre medizinischen Dokumente zu verstehen. \
         Bei gesundheitlichen Bedenken ist Ihr Arzt die beste Ansprechperson."
            .to_string()
    } else if has_prescriptive {
        "Ich kann Ihnen helfen, Ihre Dokumente zu verstehen, bin aber nicht in der Lage, \
         Behandlungen oder Maßnahmen zu empfehlen. Ihr Arzt kann Ihnen dabei helfen. \
         Möchten Sie, dass ich Ihnen helfe, eine Frage für Ihren nächsten Termin vorzubereiten?"
            .to_string()
    } else if has_diagnostic {
        "Ich kann Ihnen mitteilen, was Ihre Dokumente besagen, bin aber nicht in der Lage, Diagnosen zu stellen. \
         Möchten Sie, dass ich Ihnen erkläre, was Ihre Dokumente erwähnen?"
            .to_string()
    } else {
        "Ich kann Ihnen helfen, Ihre medizinischen Dokumente zu verstehen. \
         Könnten Sie Ihre Frage zu Ihren Dokumenten umformulieren?"
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::safety::types::FilterLayer;

    fn diagnostic_violation() -> Violation {
        Violation {
            layer: FilterLayer::KeywordScan,
            category: ViolationCategory::DiagnosticLanguage,
            matched_text: "you have diabetes".into(),
            offset: 0,
            length: 17,
            reason: "test".into(),
        }
    }

    fn prescriptive_violation() -> Violation {
        Violation {
            layer: FilterLayer::KeywordScan,
            category: ViolationCategory::PrescriptiveLanguage,
            matched_text: "you should take".into(),
            offset: 0,
            length: 15,
            reason: "test".into(),
        }
    }

    fn alarm_violation() -> Violation {
        Violation {
            layer: FilterLayer::KeywordScan,
            category: ViolationCategory::AlarmLanguage,
            matched_text: "dangerous".into(),
            offset: 0,
            length: 9,
            reason: "test".into(),
        }
    }

    // =================================================================
    // REPHRASING
    // =================================================================

    #[test]
    fn rephrase_diagnostic_to_document_attributed() {
        let text = "You have diabetes.";
        let rephrased = rephrase_violations(text, &[diagnostic_violation()]);
        assert!(rephrased.is_some());
        let result = rephrased.unwrap();
        assert!(!result.to_lowercase().contains("you have diabetes"));
        assert!(result.to_lowercase().contains("documents") || result.to_lowercase().contains("mention"));
    }

    #[test]
    fn rephrase_prescriptive_to_discuss() {
        let text = "You should stop taking ibuprofen.";
        let rephrased = rephrase_violations(text, &[prescriptive_violation()]);
        assert!(rephrased.is_some());
        let result = rephrased.unwrap();
        assert!(
            result.to_lowercase().contains("doctor")
                || result.to_lowercase().contains("healthcare provider")
                || result.to_lowercase().contains("discuss")
        );
    }

    #[test]
    fn rephrase_alarm_to_calm() {
        // Two separate alarm violations — each gets its own rephrase rule
        let v1 = Violation {
            layer: FilterLayer::KeywordScan,
            category: ViolationCategory::AlarmLanguage,
            matched_text: "dangerous".into(),
            offset: 8,
            length: 9,
            reason: "test".into(),
        };
        let v2 = Violation {
            layer: FilterLayer::KeywordScan,
            category: ViolationCategory::AlarmLanguage,
            matched_text: "life-threatening".into(),
            offset: 22,
            length: 16,
            reason: "test".into(),
        };
        let text = "This is dangerous and life-threatening.";
        let rephrased = rephrase_violations(text, &[v1, v2]);
        assert!(rephrased.is_some());
        let result = rephrased.unwrap();
        assert!(!result.to_lowercase().contains("dangerous"));
        assert!(!result.to_lowercase().contains("life-threatening"));
    }

    #[test]
    fn clean_text_unchanged() {
        let text = "Your documents show metformin 500mg twice daily.";
        let rephrased = rephrase_violations(text, &[]);
        assert_eq!(rephrased.unwrap(), text);
    }

    #[test]
    fn no_matching_rule_returns_none() {
        let unmatchable = Violation {
            layer: FilterLayer::ReportingVsStating,
            category: ViolationCategory::UngroundedClaim,
            matched_text: "completely unique text that no rule matches".into(),
            offset: 0,
            length: 10,
            reason: "test".into(),
        };
        let result = rephrase_violations("completely unique text that no rule matches", &[unmatchable]);
        assert!(result.is_none());
    }

    // =================================================================
    // FALLBACK MESSAGES
    // =================================================================

    #[test]
    fn fallback_alarm_is_calm() {
        let msg = select_fallback_message(&[alarm_violation()]);
        assert!(!msg.to_lowercase().contains("emergency"));
        assert!(!msg.to_lowercase().contains("immediately"));
        assert!(msg.contains("healthcare provider"));
    }

    #[test]
    fn fallback_prescriptive_offers_appointment() {
        let msg = select_fallback_message(&[prescriptive_violation()]);
        assert!(msg.contains("appointment") || msg.contains("healthcare provider"));
    }

    #[test]
    fn fallback_diagnostic_offers_docs() {
        let msg = select_fallback_message(&[diagnostic_violation()]);
        assert!(msg.contains("documents"));
    }

    #[test]
    fn fallback_alarm_takes_priority() {
        let msg = select_fallback_message(&[alarm_violation(), diagnostic_violation()]);
        assert!(msg.contains("healthcare provider"));
    }

    // =================================================================
    // I18N-06: TRANSLATED FALLBACK MESSAGES
    // =================================================================

    #[test]
    fn fallback_fr_alarm() {
        let msg = select_fallback_message_i18n(&[alarm_violation()], "fr");
        assert!(msg.contains("professionnel de santé"), "French alarm fallback: {msg}");
        assert!(!msg.contains("emergency"));
    }

    #[test]
    fn fallback_fr_prescriptive() {
        let msg = select_fallback_message_i18n(&[prescriptive_violation()], "fr");
        assert!(msg.contains("rendez-vous") || msg.contains("professionnel"), "French prescriptive fallback: {msg}");
    }

    #[test]
    fn fallback_fr_diagnostic() {
        let msg = select_fallback_message_i18n(&[diagnostic_violation()], "fr");
        assert!(msg.contains("documents"), "French diagnostic fallback: {msg}");
    }

    #[test]
    fn fallback_de_alarm() {
        let msg = select_fallback_message_i18n(&[alarm_violation()], "de");
        assert!(msg.contains("Arzt"), "German alarm fallback: {msg}");
        assert!(!msg.contains("emergency"));
    }

    #[test]
    fn fallback_de_prescriptive() {
        let msg = select_fallback_message_i18n(&[prescriptive_violation()], "de");
        assert!(msg.contains("Termin") || msg.contains("Arzt"), "German prescriptive fallback: {msg}");
    }

    #[test]
    fn fallback_de_diagnostic() {
        let msg = select_fallback_message_i18n(&[diagnostic_violation()], "de");
        assert!(msg.contains("Dokumente"), "German diagnostic fallback: {msg}");
    }

    #[test]
    fn fallback_unknown_lang_defaults_to_en() {
        let msg = select_fallback_message_i18n(&[alarm_violation()], "ja");
        assert!(msg.contains("healthcare provider"), "Unknown lang should fallback to EN: {msg}");
    }

    #[test]
    fn fallback_en_matches_default() {
        let default = select_fallback_message(&[alarm_violation()]);
        let explicit = select_fallback_message_i18n(&[alarm_violation()], "en");
        assert_eq!(default, explicit);
    }
}
