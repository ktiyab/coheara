//! Post-LLM output sanitization (Spec 44 [SF-04], [SF-06]).
//!
//! Strips model artifacts (thinking tags, unused tokens) and detects
//! response truncation. Runs BEFORE the safety filter layers.

use std::sync::LazyLock;
use regex::Regex;

/// Strip model-specific artifacts from raw LLM output.
///
/// Handles:
/// 1. MedGemma thinking tags (`<unusedN>thought\n...`)
/// 2. Stray `<unusedN>` tokens from the Gemma3 tokenizer
/// 3. Leading/trailing whitespace from stripping
pub fn sanitize_llm_output(raw: &str) -> String {
    let mut text = raw.to_string();

    // 1. Strip thinking tags: <unusedN>thought\n...
    // The thinking block starts with <unused\d+>thought and ends at
    // the next non-thinking content. We remove the entire prefix.
    if let Some(idx) = text.find("<unused") {
        if let Some(thought_offset) = text[idx..].find("thought\n") {
            text = text[idx + thought_offset + 8..].to_string();
        }
    }

    // 2. Strip any remaining <unusedN> tokens
    static UNUSED_TOKEN_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"<unused\d+>").expect("valid regex"));
    text = UNUSED_TOKEN_RE.replace_all(&text, "").to_string();

    // 3. Clean up
    text.trim().to_string()
}

/// Detect if a response appears to be truncated mid-content.
///
/// Uses heuristics: missing terminal punctuation, suspiciously short
/// trailing list items, etc.
pub fn is_likely_truncated(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    let last_char = match trimmed.chars().last() {
        Some(c) => c,
        None => return false,
    };

    // Ends mid-word (no terminal punctuation)
    let has_terminal = matches!(last_char, '.' | '!' | '?' | ':' | '"' | ')' | ']');
    if !has_terminal {
        return true;
    }

    // Ends with a suspiciously short list marker
    if let Some(last_line) = trimmed.lines().last() {
        let stripped = last_line.trim();
        if (stripped.starts_with('-') || stripped.starts_with('*')) && stripped.len() < 20 {
            return true;
        }
    }

    false
}

/// I18n key for truncation disclaimer.
pub fn truncation_disclaimer(lang: &str) -> &'static str {
    match lang {
        "fr" => "Cette réponse peut être incomplète. Pour des informations complètes, \
                 veuillez consulter votre professionnel de santé.",
        "de" => "Diese Antwort ist möglicherweise unvollständig. Für umfassende Informationen \
                 wenden Sie sich bitte an Ihren Arzt.",
        _ => "This response may be incomplete. For comprehensive information about this topic, \
              please consult your healthcare provider.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── sanitize_llm_output ────────────────────────────────────

    #[test]
    fn strips_thinking_tags() {
        let raw = "<unused94>thought\nLet me think about this fever...\nThe answer is:\nFever is common.";
        let result = sanitize_llm_output(raw);
        assert_eq!(result, "Let me think about this fever...\nThe answer is:\nFever is common.");
    }

    #[test]
    fn strips_unused_tokens() {
        let raw = "Some text <unused12> and more <unused0> end.";
        let result = sanitize_llm_output(raw);
        assert_eq!(result, "Some text  and more  end.");
    }

    #[test]
    fn strips_both_thinking_and_unused() {
        let raw = "<unused94>thought\nInternal reasoning\nHere is <unused5> the answer.";
        let result = sanitize_llm_output(raw);
        assert_eq!(result, "Internal reasoning\nHere is  the answer.");
    }

    #[test]
    fn clean_text_unchanged() {
        let text = "Your documents show metformin 500mg prescribed.";
        let result = sanitize_llm_output(text);
        assert_eq!(result, text);
    }

    #[test]
    fn empty_input_returns_empty() {
        assert_eq!(sanitize_llm_output(""), "");
    }

    #[test]
    fn whitespace_only_returns_empty() {
        assert_eq!(sanitize_llm_output("   \n  "), "");
    }

    // ── is_likely_truncated ────────────────────────────────────

    #[test]
    fn complete_sentence_not_truncated() {
        assert!(!is_likely_truncated("This is a complete sentence."));
    }

    #[test]
    fn sentence_ending_question_not_truncated() {
        assert!(!is_likely_truncated("Is this complete?"));
    }

    #[test]
    fn mid_word_is_truncated() {
        assert!(is_likely_truncated("This sentence ends mid"));
    }

    #[test]
    fn short_bullet_is_truncated() {
        assert!(is_likely_truncated("- Item one\n- Ite"));
    }

    #[test]
    fn empty_not_truncated() {
        assert!(!is_likely_truncated(""));
    }

    #[test]
    fn full_list_not_truncated() {
        assert!(!is_likely_truncated("- Item one is complete.\n- Item two is also complete."));
    }

    // ── truncation_disclaimer ──────────────────────────────────

    #[test]
    fn disclaimer_en() {
        let msg = truncation_disclaimer("en");
        assert!(msg.contains("incomplete"));
    }

    #[test]
    fn disclaimer_fr() {
        let msg = truncation_disclaimer("fr");
        assert!(msg.contains("incomplète"));
    }

    #[test]
    fn disclaimer_de() {
        let msg = truncation_disclaimer("de");
        assert!(msg.contains("unvollständig"));
    }

    #[test]
    fn disclaimer_unknown_defaults_en() {
        let msg = truncation_disclaimer("ja");
        assert!(msg.contains("incomplete"));
    }
}
