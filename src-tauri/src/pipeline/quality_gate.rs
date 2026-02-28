//! L6-13: OutputQualityGate — post-generation diversity validator.
//!
//! Catches degenerate output that StreamGuard missed. StreamGuard detects
//! exact sequence repetition in the token stream. QualityGate detects
//! semantically degenerate output after generation completes: low lexical
//! diversity, excessive line repetition, or suspiciously short responses
//! for the expected output.
//!
//! Evidence: V-FR-03 incident — varied but meaningless repetition
//! ("Lab Pharmacist: Dr. LEVANDIER" × 200) passed StreamGuard because
//! the token sequence length didn't align with the repetition period.
//! C1-FIX addresses the StreamGuard side. QualityGate is defense-in-depth.

use std::collections::HashSet;
use std::fmt;

// ═══════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════

/// Configuration for the output quality gate.
///
/// Default values calibrated from BM-04/05/06 benchmark data:
/// - Normal medical output has diversity ratio > 0.4
/// - Degenerate output typically has diversity ratio < 0.15
#[derive(Debug, Clone)]
pub struct QualityGateConfig {
    /// Minimum ratio of unique tokens to total tokens.
    /// Below this threshold, the output is considered degenerate.
    /// Default: 0.15 (15% unique tokens).
    pub min_diversity_ratio: f32,
    /// Maximum ratio of the most-repeated line to total lines.
    /// If any single line accounts for more than this fraction, flag it.
    /// Default: 0.5 (50% of lines are the same line).
    pub max_line_dominance: f32,
    /// Minimum word count for the quality check to apply.
    /// Very short responses (< threshold) skip the diversity check
    /// since they naturally have high or low diversity by chance.
    /// Default: 20 words.
    pub min_words_for_check: usize,
}

impl Default for QualityGateConfig {
    fn default() -> Self {
        Self {
            min_diversity_ratio: 0.15,
            max_line_dominance: 0.5,
            min_words_for_check: 20,
        }
    }
}

// ═══════════════════════════════════════════════════════════
// Error type
// ═══════════════════════════════════════════════════════════

/// Why the output failed the quality gate.
#[derive(Debug, Clone)]
pub enum QualityGateFailure {
    /// Lexical diversity below threshold.
    LowDiversity {
        /// Ratio of unique tokens to total tokens.
        diversity_ratio: f32,
        /// Configured threshold that was violated.
        threshold: f32,
    },
    /// A single line dominates the output.
    LineDominance {
        /// The dominant line (truncated to 100 chars).
        dominant_line: String,
        /// Fraction of total lines that are this line.
        dominance_ratio: f32,
        /// Configured threshold that was violated.
        threshold: f32,
    },
}

impl fmt::Display for QualityGateFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LowDiversity {
                diversity_ratio,
                threshold,
            } => write!(
                f,
                "low_diversity(ratio={diversity_ratio:.2}, threshold={threshold:.2})"
            ),
            Self::LineDominance {
                dominant_line,
                dominance_ratio,
                threshold,
            } => {
                let preview = if dominant_line.len() > 100 {
                    &dominant_line[..100]
                } else {
                    dominant_line
                };
                write!(
                    f,
                    "line_dominance(\"{preview}\" at {dominance_ratio:.2}, threshold={threshold:.2})"
                )
            }
        }
    }
}

impl std::error::Error for QualityGateFailure {}

// ═══════════════════════════════════════════════════════════
// Validation
// ═══════════════════════════════════════════════════════════

/// Validate LLM output for quality indicators.
///
/// Returns `Ok(())` if the output passes all checks.
/// Returns `Err(QualityGateFailure)` if degeneration is detected.
///
/// Checks are applied only if the output has enough words
/// (`min_words_for_check`). Short responses pass automatically.
pub fn validate_output(
    text: &str,
    config: &QualityGateConfig,
) -> Result<(), QualityGateFailure> {
    let words: Vec<&str> = text.split_whitespace().collect();

    // Short responses skip the check — not enough signal
    if words.len() < config.min_words_for_check {
        return Ok(());
    }

    // Check 1: Lexical diversity (unique words / total words)
    let unique: HashSet<&str> = words.iter().copied().collect();
    let diversity_ratio = unique.len() as f32 / words.len() as f32;

    if diversity_ratio < config.min_diversity_ratio {
        return Err(QualityGateFailure::LowDiversity {
            diversity_ratio,
            threshold: config.min_diversity_ratio,
        });
    }

    // Check 2: Line dominance (most-repeated line / total lines)
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    if lines.len() >= 3 {
        let mut line_counts: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for line in &lines {
            *line_counts.entry(line).or_insert(0) += 1;
        }

        if let Some((&dominant_line, &count)) = line_counts.iter().max_by_key(|(_, &c)| c) {
            let dominance_ratio = count as f32 / lines.len() as f32;
            if dominance_ratio > config.max_line_dominance {
                return Err(QualityGateFailure::LineDominance {
                    dominant_line: dominant_line.to_string(),
                    dominance_ratio,
                    threshold: config.max_line_dominance,
                });
            }
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> QualityGateConfig {
        QualityGateConfig::default()
    }

    // ── Healthy output ──────────────────────────────────

    #[test]
    fn healthy_medical_text_passes() {
        let text = "The patient presented with persistent headaches \
                    over the past two weeks. Blood pressure was measured \
                    at 140/90 mmHg. Lab results show elevated creatinine \
                    at 1.8 mg/dL. Prescribed Lisinopril 10mg daily.";
        assert!(validate_output(text, &default_config()).is_ok());
    }

    #[test]
    fn healthy_lab_results_pass() {
        let text = "- Hemoglobin: 13.3 g/dl (13.4-16.7)\n\
                    - Hematocrit: 41.0% (39.0-49.0)\n\
                    - Platelets: 250,000 /mm3 (150,000-400,000)\n\
                    - White blood cells: 6,160 /mm3 (4,000-11,000)\n\
                    - Red blood cells: 4.88 T/l (4.28-6.00)\n\
                    - MCV: 84 fl (78-98)\n\
                    - MCH: 27.3 pg (26.0-34.0)";
        assert!(validate_output(text, &default_config()).is_ok());
    }

    #[test]
    fn short_response_passes_automatically() {
        // Below min_words_for_check (20), so quality gate skips
        let text = "4.88 T/l";
        assert!(validate_output(text, &default_config()).is_ok());
    }

    #[test]
    fn empty_text_passes() {
        assert!(validate_output("", &default_config()).is_ok());
    }

    // ── Low diversity detection ─────────────────────────

    #[test]
    fn degenerate_repetition_caught() {
        // Simulate V-FR-03: same phrase repeated many times
        let text = (0..50)
            .map(|_| "* Lab Pharmacist: Dr. LEVANDIER")
            .collect::<Vec<_>>()
            .join("\n");
        let result = validate_output(&text, &default_config());
        // May trigger LowDiversity or LineDominance — either is correct
        assert!(result.is_err(), "V-FR-03 pattern must be caught by quality gate");
    }

    #[test]
    fn single_word_spam_caught() {
        let text = vec!["error"; 100].join(" ");
        let result = validate_output(&text, &default_config());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QualityGateFailure::LowDiversity { .. }
        ));
    }

    #[test]
    fn varied_but_repetitive_caught() {
        // Different words but very low diversity ratio
        let text = (0..30)
            .map(|_| "the patient took medication daily for pain relief")
            .collect::<Vec<_>>()
            .join(" ");
        let result = validate_output(&text, &default_config());
        assert!(result.is_err());
    }

    // ── Line dominance detection ────────────────────────

    #[test]
    fn line_dominance_caught() {
        let mut lines = vec![
            "Medication: Paracetamol 500mg",
            "Medication: Paracetamol 500mg",
            "Medication: Paracetamol 500mg",
            "Medication: Paracetamol 500mg",
            "Medication: Paracetamol 500mg",
            "Medication: Paracetamol 500mg",
            "Medication: Paracetamol 500mg",
            "Medication: Paracetamol 500mg",
            "Medication: Paracetamol 500mg",
            "Medication: Paracetamol 500mg",
        ];
        lines.push("Diagnosis: Hypertension");
        let text = lines.join("\n");
        let result = validate_output(&text, &default_config());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            QualityGateFailure::LineDominance { .. }
        ));
    }

    #[test]
    fn moderate_repetition_passes() {
        // 2 out of 6 lines are the same — 33%, below 50% threshold
        let text = "Medication: Paracetamol 500mg\n\
                    Medication: Ibuprofen 400mg\n\
                    Medication: Paracetamol 500mg\n\
                    Lab: Hemoglobin 13.3 g/dl\n\
                    Lab: Hematocrit 41.0%\n\
                    Diagnosis: Essential hypertension";
        assert!(validate_output(&text, &default_config()).is_ok());
    }

    // ── Config override ─────────────────────────────────

    #[test]
    fn strict_config_catches_more() {
        let config = QualityGateConfig {
            min_diversity_ratio: 0.5,
            max_line_dominance: 0.3,
            min_words_for_check: 10,
        };
        // This passes with default config but fails with strict
        let text = "take one tablet daily with food take one tablet daily \
                    with food take one tablet daily with food and water";
        let result = validate_output(&text, &config);
        assert!(result.is_err());
    }

    #[test]
    fn lenient_config_allows_more() {
        let config = QualityGateConfig {
            min_diversity_ratio: 0.05,
            max_line_dominance: 0.9,
            min_words_for_check: 50,
        };
        let text = vec!["error error error"; 10].join(" ");
        // With lenient config, this passes (diversity check has low threshold)
        assert!(validate_output(&text, &config).is_ok());
    }

    // ── Display formatting ──────────────────────────────

    #[test]
    fn low_diversity_display() {
        let failure = QualityGateFailure::LowDiversity {
            diversity_ratio: 0.08,
            threshold: 0.15,
        };
        let s = format!("{failure}");
        assert!(s.contains("low_diversity"));
        assert!(s.contains("0.08"));
    }

    #[test]
    fn line_dominance_display() {
        let failure = QualityGateFailure::LineDominance {
            dominant_line: "test line".to_string(),
            dominance_ratio: 0.75,
            threshold: 0.5,
        };
        let s = format!("{failure}");
        assert!(s.contains("line_dominance"));
        assert!(s.contains("test line"));
    }

    // ── Edge cases ──────────────────────────────────────

    #[test]
    fn single_line_with_many_words_passes() {
        // One very long line with good diversity — should pass
        let text = "The comprehensive blood panel revealed normal hemoglobin \
                    levels at 14.2 g/dL with adequate platelet count of 245000 \
                    and white blood cell count within reference range";
        assert!(validate_output(&text, &default_config()).is_ok());
    }

    #[test]
    fn two_lines_skip_dominance_check() {
        // Only 2 non-empty lines — line dominance needs >= 3
        let text = "test line repeated many times for the quality check\n\
                    test line repeated many times for the quality check";
        // Low diversity check may catch this, but line dominance is skipped
        let result = validate_output(&text, &default_config());
        // With 2 identical lines, diversity ratio is about 8/16 = 0.5, which passes
        assert!(result.is_ok());
    }

    #[test]
    fn unicode_medical_text_passes() {
        let text = "Résultats d'analyse pour le patient. \
                    Hémoglobine mesurée à 13.3 g/dl, légèrement inférieure \
                    à la valeur de référence de 13.4 g/dl. Hématocrite \
                    normal à 41.0 pourcent. Leucocytes dans les limites.";
        assert!(validate_output(&text, &default_config()).is_ok());
    }
}
