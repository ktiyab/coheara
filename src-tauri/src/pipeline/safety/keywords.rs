use std::sync::LazyLock;

use regex::Regex;

use super::types::{FilterLayer, Violation, ViolationCategory};

/// A compiled pattern with its violation metadata.
struct SafetyPattern {
    regex: Regex,
    category: ViolationCategory,
    description: &'static str,
}

/// All diagnostic language patterns (Layer 2) — 8 patterns per spec.
static DIAGNOSTIC_PATTERNS: LazyLock<Vec<SafetyPattern>> = LazyLock::new(|| {
    vec![
        pattern(
            r"(?i)\byou\s+have\s+(?:a\s+)?(?:been\s+)?(?:diagnosed\s+with\s+)?[a-z]",
            ViolationCategory::DiagnosticLanguage,
            "Direct diagnosis: 'you have [condition]'",
        ),
        pattern(
            r"(?i)\byou\s+are\s+suffering\s+from\b",
            ViolationCategory::DiagnosticLanguage,
            "Direct diagnosis: 'you are suffering from'",
        ),
        pattern(
            r"(?i)\byou\s+(?:likely|probably|possibly)\s+have\b",
            ViolationCategory::DiagnosticLanguage,
            "Speculative diagnosis: 'you likely/probably have'",
        ),
        pattern(
            r"(?i)\bthis\s+(?:means|indicates|suggests|confirms)\s+(?:you|that\s+you)\s+have\b",
            ViolationCategory::DiagnosticLanguage,
            "Indirect diagnosis: 'this means you have'",
        ),
        pattern(
            r"(?i)\byou\s+(?:are|have\s+been)\s+diagnosed\b",
            ViolationCategory::DiagnosticLanguage,
            "Diagnosis claim without document attribution",
        ),
        pattern(
            r"(?i)\byou(?:'re|\s+are)\s+(?:a\s+)?diabetic\b",
            ViolationCategory::DiagnosticLanguage,
            "Direct label: 'you are diabetic'",
        ),
        pattern(
            r"(?i)\byour\s+condition\s+is\b",
            ViolationCategory::DiagnosticLanguage,
            "Condition assertion: 'your condition is'",
        ),
        pattern(
            r"(?i)\byou\s+(?:appear|seem)\s+to\s+have\b",
            ViolationCategory::DiagnosticLanguage,
            "Implied diagnosis: 'you appear to have'",
        ),
    ]
});

/// All prescriptive language patterns (Layer 2) — 8 patterns per spec.
static PRESCRIPTIVE_PATTERNS: LazyLock<Vec<SafetyPattern>> = LazyLock::new(|| {
    vec![
        pattern(
            r"(?i)\byou\s+should\s+(?:take|stop|start|increase|decrease|change|switch|discontinue|avoid|reduce)\b",
            ViolationCategory::PrescriptiveLanguage,
            "Direct prescription: 'you should [take/stop/...]'",
        ),
        pattern(
            r"(?i)\bI\s+recommend\b",
            ViolationCategory::PrescriptiveLanguage,
            "Direct recommendation: 'I recommend'",
        ),
        pattern(
            r"(?i)\bI\s+(?:would\s+)?(?:suggest|advise)\b",
            ViolationCategory::PrescriptiveLanguage,
            "Advisory language: 'I suggest/advise'",
        ),
        pattern(
            r"(?i)\byou\s+(?:need|must|have)\s+to\s+(?:take|stop|start|see|visit|go|call|increase|decrease)\b",
            ViolationCategory::PrescriptiveLanguage,
            "Imperative prescription: 'you need to [action]'",
        ),
        pattern(
            r"(?i)\bdo\s+not\s+(?:take|stop|eat|drink|use|skip)\b",
            ViolationCategory::PrescriptiveLanguage,
            "Prohibition: 'do not [action]'",
        ),
        pattern(
            r"(?i)\btry\s+(?:taking|using|adding|reducing)\b",
            ViolationCategory::PrescriptiveLanguage,
            "Soft prescription: 'try taking/using'",
        ),
        pattern(
            r"(?i)\bthe\s+(?:best|recommended)\s+(?:treatment|course\s+of\s+action|approach)\s+(?:is|would\s+be)\b",
            ViolationCategory::PrescriptiveLanguage,
            "Treatment recommendation: 'the best treatment is'",
        ),
        pattern(
            r"(?i)\bconsider\s+(?:taking|stopping|increasing|decreasing|switching)\b",
            ViolationCategory::PrescriptiveLanguage,
            "Soft prescription: 'consider taking/stopping'",
        ),
    ]
});

/// All alarm/emergency language patterns (Layer 2) — 8 patterns per spec.
/// NC-07: Calm design language. No alarm wording. No red alerts.
static ALARM_PATTERNS: LazyLock<Vec<SafetyPattern>> = LazyLock::new(|| {
    vec![
        pattern(
            r"(?i)\b(?:dangerous|life[- ]threatening|fatal|deadly|lethal)\b",
            ViolationCategory::AlarmLanguage,
            "Alarm word: dangerous/life-threatening/fatal",
        ),
        pattern(
            r"(?i)\b(?:emergency|urgent(?:ly)?|immediately|right\s+away|right\s+now)\b",
            ViolationCategory::AlarmLanguage,
            "Urgency word: emergency/immediately/urgently",
        ),
        pattern(
            r"(?i)\b(?:immediately|urgently)\s+(?:go|call|visit|see|seek|get)\b",
            ViolationCategory::AlarmLanguage,
            "Urgent directive: 'immediately go/call'",
        ),
        pattern(
            r"(?i)\bcall\s+(?:911|emergency|an\s+ambulance|your\s+doctor\s+(?:immediately|right\s+away|now))\b",
            ViolationCategory::AlarmLanguage,
            "Emergency call directive: 'call 911/emergency'",
        ),
        pattern(
            r"(?i)\bgo\s+to\s+(?:the\s+)?(?:emergency|ER|hospital|A&E)\b",
            ViolationCategory::AlarmLanguage,
            "ER directive: 'go to the emergency/hospital'",
        ),
        pattern(
            r"(?i)\bseek\s+(?:immediate|emergency|urgent)\s+(?:medical\s+)?(?:help|attention|care)\b",
            ViolationCategory::AlarmLanguage,
            "Seek care directive: 'seek immediate medical help'",
        ),
        pattern(
            r"(?i)\bthis\s+(?:is|could\s+be)\s+(?:a\s+)?(?:medical\s+)?emergency\b",
            ViolationCategory::AlarmLanguage,
            "Emergency declaration: 'this is an emergency'",
        ),
        pattern(
            r"(?i)\bdo\s+not\s+(?:wait|delay|ignore)\b",
            ViolationCategory::AlarmLanguage,
            "Urgency pressure: 'do not wait/delay'",
        ),
    ]
});

fn pattern(regex_str: &str, category: ViolationCategory, description: &'static str) -> SafetyPattern {
    SafetyPattern {
        regex: Regex::new(regex_str).expect("Invalid safety regex pattern"),
        category,
        description,
    }
}

/// Layer 2: Scan response text for diagnostic, prescriptive, and alarm language.
pub fn scan_keywords(text: &str) -> Vec<Violation> {
    let mut violations = Vec::new();

    for patterns in [&*DIAGNOSTIC_PATTERNS, &*PRESCRIPTIVE_PATTERNS, &*ALARM_PATTERNS] {
        for sp in patterns {
            for mat in sp.regex.find_iter(text) {
                violations.push(Violation {
                    layer: FilterLayer::KeywordScan,
                    category: sp.category.clone(),
                    matched_text: mat.as_str().to_string(),
                    offset: mat.start(),
                    length: mat.len(),
                    reason: sp.description.to_string(),
                });
            }
        }
    }

    deduplicate_violations(&mut violations);

    violations
}

/// Remove overlapping violations, keeping the more specific (longer) match.
pub fn deduplicate_violations(violations: &mut Vec<Violation>) {
    violations.sort_by_key(|v| (v.offset, std::cmp::Reverse(v.length)));
    let mut i = 0;
    while i < violations.len() {
        let mut j = i + 1;
        while j < violations.len() {
            let vi_end = violations[i].offset + violations[i].length;
            let vj_end = violations[j].offset + violations[j].length;
            // If vj is fully contained within vi, remove vj
            if violations[j].offset >= violations[i].offset && vj_end <= vi_end {
                violations.remove(j);
            } else {
                j += 1;
            }
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =================================================================
    // LAYER 2: KEYWORD SCAN — DIAGNOSTIC
    // =================================================================

    #[test]
    fn keyword_you_have_diabetes() {
        let violations = scan_keywords("Based on the symptoms, you have diabetes.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn keyword_you_are_suffering_from() {
        let violations = scan_keywords("You are suffering from chronic pain.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn keyword_you_likely_have() {
        let violations = scan_keywords("You likely have an infection.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn keyword_you_are_diabetic() {
        let violations = scan_keywords("Since you're diabetic, watch your sugar.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn keyword_your_condition_is() {
        let violations = scan_keywords("Your condition is worsening over time.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    #[test]
    fn keyword_you_appear_to_have() {
        let violations = scan_keywords("You appear to have a thyroid condition.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage));
    }

    // =================================================================
    // LAYER 2: KEYWORD SCAN — PRESCRIPTIVE
    // =================================================================

    #[test]
    fn keyword_you_should_take() {
        let violations = scan_keywords("You should take aspirin daily.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn keyword_you_should_stop() {
        let violations = scan_keywords("You should stop taking ibuprofen.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn keyword_i_recommend() {
        let violations = scan_keywords("I recommend starting a low-sodium diet.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn keyword_you_need_to_see() {
        let violations = scan_keywords("You need to see a specialist immediately.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn keyword_do_not_take() {
        let violations = scan_keywords("Do not take this medication with alcohol.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn keyword_try_taking() {
        let violations = scan_keywords("Try taking this supplement in the morning.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn keyword_consider_stopping() {
        let violations = scan_keywords("Consider stopping the medication before surgery.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    // =================================================================
    // LAYER 2: KEYWORD SCAN — ALARM
    // =================================================================

    #[test]
    fn keyword_dangerous() {
        let violations = scan_keywords("This interaction could be dangerous.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn keyword_immediately_go() {
        let violations = scan_keywords("Immediately go to the emergency room.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn keyword_call_911() {
        let violations = scan_keywords("Call 911 right away.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn keyword_seek_immediate_medical_attention() {
        let violations = scan_keywords("Seek immediate medical attention.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn keyword_life_threatening() {
        let violations = scan_keywords("This could be life-threatening.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn keyword_this_is_emergency() {
        let violations = scan_keywords("This is a medical emergency.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    #[test]
    fn keyword_do_not_wait() {
        let violations = scan_keywords("Do not wait to seek treatment.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage));
    }

    // =================================================================
    // CLEAN PASS + EDGE CASES
    // =================================================================

    #[test]
    fn clean_text_no_violations() {
        let violations = scan_keywords(
            "Your documents show that Dr. Chen prescribed metformin 500mg twice daily. \
             This was documented on January 15, 2024.",
        );
        assert!(violations.is_empty(), "Expected no violations, got: {:?}", violations);
    }

    #[test]
    fn case_insensitive_detection() {
        let violations_lower = scan_keywords("you should take aspirin.");
        let violations_upper = scan_keywords("You Should Take aspirin.");
        let violations_mixed = scan_keywords("YOU SHOULD TAKE aspirin.");
        assert!(!violations_lower.is_empty());
        assert!(!violations_upper.is_empty());
        assert!(!violations_mixed.is_empty());
    }

    #[test]
    fn deduplicate_overlapping() {
        let mut violations = vec![
            Violation {
                layer: FilterLayer::KeywordScan,
                category: ViolationCategory::AlarmLanguage,
                matched_text: "immediately go to the emergency".to_string(),
                offset: 0,
                length: 32,
                reason: "test".to_string(),
            },
            Violation {
                layer: FilterLayer::KeywordScan,
                category: ViolationCategory::AlarmLanguage,
                matched_text: "immediately go".to_string(),
                offset: 0,
                length: 14,
                reason: "test".to_string(),
            },
        ];
        deduplicate_violations(&mut violations);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].length, 32);
    }

    #[test]
    fn safe_document_attribution_passes() {
        let text = "According to your documents, your doctor noted a diagnosis of hypertension. \
                     You might want to ask your doctor about any changes to your treatment plan.";
        let violations = scan_keywords(text);
        assert!(violations.is_empty(), "Got violations: {:?}", violations);
    }

    // =================================================================
    // ADVERSARIAL / RED-TEAM TESTS (RS-L2-02-001)
    // Response-side: LLM-generated text that attempts to bypass safety
    // =================================================================

    #[test]
    fn adversarial_diagnostic_with_hedge_words() {
        // LLM might try to soften diagnosis with hedge words
        let texts = [
            "You likely have an infection based on these results.",
            "You probably have type 2 diabetes.",
            "You possibly have a thyroid condition.",
            "You appear to have cardiovascular disease.",
            "You seem to have a chronic condition.",
        ];
        for text in texts {
            let violations = scan_keywords(text);
            assert!(!violations.is_empty(), "Should catch hedged diagnosis: {text}");
        }
    }

    #[test]
    fn adversarial_prescriptive_with_soft_framing() {
        // LLM might frame prescriptions as suggestions
        let texts = [
            "Consider taking this supplement daily.",
            "Try taking the medication in the morning.",
            "The best treatment would be antibiotics.",
            "The recommended course of action is surgery.",
        ];
        for text in texts {
            let violations = scan_keywords(text);
            assert!(!violations.is_empty(), "Should catch soft prescription: {text}");
        }
    }

    #[test]
    fn adversarial_alarm_with_medical_terminology() {
        // LLM might use clinical urgency language
        let texts = [
            "This could be life threatening if left untreated.",
            "This is a medical emergency requiring immediate attention.",
            "Seek urgent medical care for this condition.",
            "Go to the ER as soon as possible.",
            "Do not delay treatment any further.",
        ];
        for text in texts {
            let violations = scan_keywords(text);
            assert!(!violations.is_empty(), "Should catch clinical alarm: {text}");
        }
    }

    #[test]
    fn adversarial_multi_violation_response() {
        // LLM response that hits all three categories
        let text = "You have diabetes. You should take metformin. \
                    This is dangerous and you should immediately go to the hospital.";
        let violations = scan_keywords(text);
        assert!(
            violations.iter().any(|v| v.category == ViolationCategory::DiagnosticLanguage),
            "Should detect diagnostic violation"
        );
        assert!(
            violations.iter().any(|v| v.category == ViolationCategory::PrescriptiveLanguage),
            "Should detect prescriptive violation"
        );
        assert!(
            violations.iter().any(|v| v.category == ViolationCategory::AlarmLanguage),
            "Should detect alarm violation"
        );
    }

    #[test]
    fn adversarial_embedded_in_long_safe_text() {
        // Single violation buried in long otherwise-safe text
        let text = "Your documents from January 2024 show several lab results. \
                    Dr. Chen documented your hemoglobin at 13.5 g/dL, which is within \
                    the normal reference range of 12.0-17.5 g/dL. Your cholesterol panel \
                    showed total cholesterol at 210 mg/dL. You should stop taking the \
                    supplement before your next test. The creatinine was 1.1 mg/dL.";
        let violations = scan_keywords(text);
        assert!(!violations.is_empty(), "Should catch embedded violation");
        assert!(violations
            .iter()
            .any(|v| v.category == ViolationCategory::PrescriptiveLanguage));
    }

    #[test]
    fn adversarial_safe_responses_not_flagged() {
        // Ensure properly-framed safe responses don't trigger false positives
        let safe_texts = [
            "Your documents show that Dr. Chen prescribed metformin 500mg twice daily.",
            "According to your records, your last A1c was 7.2%.",
            "Your lab results from January indicate a hemoglobin of 13.5 g/dL.",
            "You might want to ask your healthcare provider about this result.",
            "This is something you may want to discuss with your doctor.",
        ];
        for text in safe_texts {
            let violations = scan_keywords(text);
            assert!(violations.is_empty(), "False positive on safe text: {text}");
        }
    }
}
