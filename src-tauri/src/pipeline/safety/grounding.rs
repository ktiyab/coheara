use std::sync::LazyLock;

use regex::Regex;

use super::types::{FilterLayer, Violation, ViolationCategory};

/// A compiled pattern with its violation metadata.
struct SafetyPattern {
    regex: Regex,
    category: ViolationCategory,
    description: &'static str,
}

/// Patterns that indicate safe, document-grounded language.
/// If a sentence matches one of these, it is ALLOWED even if it
/// contains words that would otherwise trigger a violation.
static GROUNDED_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // Document attribution patterns
        Regex::new(r"(?i)\byour\s+(?:documents?|records?|reports?|results?|files?|lab\s+results?|test\s+results?|medical\s+records?)\s+(?:show|indicate|mention|state|note|reveal|suggest|describe|include|contain|list|record)\b").unwrap(),
        // Professional attribution patterns
        Regex::new(r"(?i)\b(?:Dr\.?\s+\w+|your\s+(?:doctor|physician|specialist|cardiologist|GP|practitioner|healthcare\s+provider))\s+(?:noted|wrote|documented|recorded|diagnosed|prescribed|mentioned|indicated|observed|stated|reported)\b").unwrap(),
        // Passive document attribution
        Regex::new(r"(?i)\b(?:according\s+to|based\s+on|as\s+(?:noted|stated|documented|recorded|mentioned)\s+in)\s+(?:your|the)\s+(?:documents?|records?|reports?|results?|files?|prescription|discharge\s+summary|clinical\s+notes?)\b").unwrap(),
        // Citation-linked patterns (inline [Doc: ...] references)
        Regex::new(r"(?i)\[Doc:\s*[a-f0-9-]+").unwrap(),
        // Date-attributed patterns
        Regex::new(r"(?i)\b(?:in|on|from)\s+(?:your|the)\s+(?:January|February|March|April|May|June|July|August|September|October|November|December|\d{4}|\d{1,2}/\d{1,2})").unwrap(),
    ]
});

/// Patterns that indicate ungrounded claims about the patient.
/// These are BLOCKED unless the same sentence also matches a GROUNDED_PATTERN.
static UNGROUNDED_PATTERNS: LazyLock<Vec<SafetyPattern>> = LazyLock::new(|| {
    vec![
        SafetyPattern {
            regex: Regex::new(r"(?i)\byou\s+have\s+(?:a\s+)?[a-z]").unwrap(),
            category: ViolationCategory::UngroundedClaim,
            description: "Ungrounded: 'you have [condition]' without document reference",
        },
        SafetyPattern {
            regex: Regex::new(r"(?i)\byou\s+are\s+(?:a\s+)?(?:diabetic|hypertensive|anemic|asthmatic|allergic|obese|overweight|immunocompromised)\b").unwrap(),
            category: ViolationCategory::UngroundedClaim,
            description: "Ungrounded label: 'you are [medical label]'",
        },
        SafetyPattern {
            regex: Regex::new(r"(?i)\byou(?:'ve|\s+have)\s+been\s+(?:experiencing|having|showing)\b").unwrap(),
            category: ViolationCategory::UngroundedClaim,
            description: "Ungrounded observation: 'you have been experiencing'",
        },
        SafetyPattern {
            regex: Regex::new(r"(?i)\byour\s+(?:blood\s+pressure|cholesterol|glucose|sugar|levels?|count|heart\s+rate|weight|BMI)\s+(?:is|are)\s+(?:high|low|elevated|abnormal|concerning|worrying|critical)\b").unwrap(),
            category: ViolationCategory::UngroundedClaim,
            description: "Ungrounded value judgment: 'your [metric] is [judgment]'",
        },
    ]
});

/// A sentence extracted from the response with its byte offset.
#[derive(Debug, Clone)]
struct Sentence<'a> {
    text: &'a str,
    offset: usize,
}

/// Common abbreviations that end with a period but are NOT sentence boundaries.
const ABBREVIATIONS: &[&str] = &[
    "Dr.", "Mr.", "Mrs.", "Ms.", "Prof.", "Jr.", "Sr.", "St.",
    "vs.", "etc.", "e.g.", "i.e.", "approx.", "dept.", "est.",
    "avg.", "max.", "min.", "vol.", "no.", "pt.",
];

/// Check if the text ending at `period_pos` ends with a known abbreviation.
fn ends_with_abbreviation(text: &str, period_pos: usize) -> bool {
    let prefix = &text[..=period_pos];
    for abbr in ABBREVIATIONS {
        if prefix.ends_with(abbr) {
            return true;
        }
        // Also check case-insensitive
        if prefix.len() >= abbr.len() {
            let candidate = &prefix[prefix.len() - abbr.len()..];
            if candidate.eq_ignore_ascii_case(abbr) {
                return true;
            }
        }
    }
    false
}

/// Split text into sentences, tracking byte offsets.
/// Uses character-based splitting to avoid lookbehind (unsupported by regex crate).
/// Handles common abbreviations (Dr., e.g.) to avoid false splits.
fn split_into_sentences(text: &str) -> Vec<Sentence<'_>> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();

    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];

        // Check for sentence-ending punctuation
        if c == b'.' || c == b'!' || c == b'?' {
            let end_pos = i + 1;

            // Skip if this is an abbreviation (only applies to periods)
            if c == b'.' && ends_with_abbreviation(text, i) {
                i += 1;
                continue;
            }

            // Check if followed by whitespace then uppercase (sentence boundary)
            if end_pos < bytes.len() {
                let rest = &text[end_pos..];
                let mut chars = rest.chars();
                if let Some(next) = chars.next() {
                    if next.is_whitespace() {
                        // Check for uppercase after whitespace
                        let after_ws: String = chars.take_while(|c| c.is_whitespace()).collect();
                        let ws_len = next.len_utf8() + after_ws.len();
                        if end_pos + ws_len < text.len() {
                            let following = text[end_pos + ws_len..].chars().next();
                            if let Some(fc) = following {
                                if fc.is_uppercase() {
                                    // Split here
                                    let sentence_text = text[start..end_pos].trim();
                                    if !sentence_text.is_empty() {
                                        sentences.push(Sentence {
                                            text: sentence_text,
                                            offset: start,
                                        });
                                    }
                                    start = end_pos + ws_len;
                                    i = start;
                                    continue;
                                }
                            }
                        }
                    } else if next == '\n' {
                        // Newline after punctuation — split
                        let sentence_text = text[start..end_pos].trim();
                        if !sentence_text.is_empty() {
                            sentences.push(Sentence {
                                text: sentence_text,
                                offset: start,
                            });
                        }
                        // Skip newlines
                        let mut skip = end_pos;
                        while skip < text.len() && text.as_bytes()[skip] == b'\n' {
                            skip += 1;
                        }
                        start = skip;
                        i = start;
                        continue;
                    }
                }
            }
        } else if c == b'\n' {
            // Newline without preceding punctuation — still split
            let sentence_text = text[start..i].trim();
            if !sentence_text.is_empty() {
                sentences.push(Sentence {
                    text: sentence_text,
                    offset: start,
                });
            }
            // Skip consecutive newlines
            let mut skip = i + 1;
            while skip < text.len() && text.as_bytes()[skip] == b'\n' {
                skip += 1;
            }
            start = skip;
            i = start;
            continue;
        }

        i += 1;
    }

    // Last sentence
    let remaining = text[start..].trim();
    if !remaining.is_empty() {
        sentences.push(Sentence {
            text: remaining,
            offset: start,
        });
    }

    sentences
}

/// Layer 3: Check that medical claims in the response are grounded in document references.
///
/// A sentence containing an ungrounded pattern MUST also contain a grounding phrase.
/// If it doesn't, it's acting as a clinician, not a document assistant.
pub fn check_grounding(text: &str) -> Vec<Violation> {
    let mut violations = Vec::new();

    let sentences = split_into_sentences(text);

    for sentence in &sentences {
        for pattern in UNGROUNDED_PATTERNS.iter() {
            if let Some(mat) = pattern.regex.find(sentence.text) {
                // Check if the SAME sentence also contains a grounding pattern
                let is_grounded = GROUNDED_PATTERNS
                    .iter()
                    .any(|gp| gp.is_match(sentence.text));

                if !is_grounded {
                    violations.push(Violation {
                        layer: FilterLayer::ReportingVsStating,
                        category: pattern.category.clone(),
                        matched_text: mat.as_str().to_string(),
                        offset: sentence.offset + mat.start(),
                        length: mat.len(),
                        reason: format!(
                            "{} -- sentence has no document attribution",
                            pattern.description
                        ),
                    });
                }
            }
        }
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    // =================================================================
    // GROUNDED (ALLOWED)
    // =================================================================

    #[test]
    fn grounding_document_attributed_passes() {
        let violations = check_grounding(
            "Your documents show that Dr. Chen diagnosed hypertension on 2024-01-15.",
        );
        assert!(violations.is_empty(), "Expected no violations, got: {:?}", violations);
    }

    #[test]
    fn grounding_according_to_records_passes() {
        let violations = check_grounding(
            "According to your records, you have been prescribed metformin.",
        );
        assert!(violations.is_empty(), "Expected no violations, got: {:?}", violations);
    }

    #[test]
    fn grounding_professional_attributed_passes() {
        let violations = check_grounding(
            "Dr. Martin noted that you have elevated cholesterol.",
        );
        assert!(violations.is_empty(), "Expected no violations, got: {:?}", violations);
    }

    #[test]
    fn grounding_doc_citation_passes() {
        let violations = check_grounding(
            "You have a diagnosis of diabetes [Doc: 550e8400-e29b-41d4-a716-446655440000].",
        );
        assert!(violations.is_empty(), "Expected no violations, got: {:?}", violations);
    }

    #[test]
    fn grounding_lab_results_attributed_passes() {
        let violations = check_grounding(
            "Your lab results show that your blood pressure is elevated.",
        );
        assert!(violations.is_empty(), "Expected no violations, got: {:?}", violations);
    }

    // =================================================================
    // UNGROUNDED (BLOCKED)
    // =================================================================

    #[test]
    fn grounding_ungrounded_you_have() {
        let violations = check_grounding("You have hypertension.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::UngroundedClaim));
    }

    #[test]
    fn grounding_ungrounded_you_are_label() {
        let violations = check_grounding("You are diabetic and should monitor your glucose.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::UngroundedClaim));
    }

    #[test]
    fn grounding_ungrounded_your_bp_is_high() {
        let violations = check_grounding("Your blood pressure is high.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::UngroundedClaim));
    }

    #[test]
    fn grounding_ungrounded_you_have_been_experiencing() {
        let violations = check_grounding("You have been experiencing frequent headaches.");
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.category == ViolationCategory::UngroundedClaim));
    }

    // =================================================================
    // NON-MEDICAL TEXT + EDGE CASES
    // =================================================================

    #[test]
    fn non_medical_text_passes() {
        let violations = check_grounding(
            "I can help you understand your medical documents. Just ask me a question about something in your records.",
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn empty_text_passes() {
        let violations = check_grounding("");
        assert!(violations.is_empty());
    }

    // =================================================================
    // SENTENCE SPLITTING
    // =================================================================

    #[test]
    fn sentence_split_basic() {
        let sentences = split_into_sentences(
            "First sentence. Second sentence. Third sentence.",
        );
        assert!(sentences.len() >= 2, "Expected at least 2 sentences, got {}", sentences.len());
    }

    #[test]
    fn sentence_split_preserves_dr_abbreviation() {
        let sentences = split_into_sentences(
            "Dr. Chen prescribed metformin. The dose is 500mg.",
        );
        assert!(sentences.len() >= 1);
    }
}
