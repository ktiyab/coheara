//! L6-05: Strategy Service — context-aware extraction strategy resolution.
//!
//! Maps (ContextType, ModelVariant) to a PromptStrategy that determines which
//! extraction approach the pipeline uses. Decision matrix is hardcoded from
//! product decisions + BM-04/05/06 benchmark data.
//!
//! Evidence: MF-37 (markdown list optimal), MF-44 (prompt complexity dominant),
//! MF-23 (45% GPU degen on JSON), BM-04/05/06.
//!
//! Amendment (2026-02-27): LiveChat renamed to HealthQuery, temperature 0.1
//! for all contexts, max_tokens 4096 for HealthQuery. See [RM-ZE-QJ], [YL-LM-XJ].
//!
//! STR-01 (2026-02-27): LegacyJson eliminated from decision matrix.
//! Principle: SLM does ONE thing per call, code orchestrates. Context alone
//! determines strategy — variant no longer affects strategy choice.
//! All contexts use element-focused extraction (MarkdownList or IterativeDrill).

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::pipeline::hardware_advisor::ModelVariant;
use crate::pipeline::prompt_templates::PromptStrategyKind;

// ═══════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════

/// Calling context that determines which strategy to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextType {
    /// Document import → full extraction pipeline.
    DocumentExtraction,
    /// User Ask session — medical research queries (not a chatbot).
    HealthQuery,
    /// Night batch extraction from conversations.
    NightBatch,
    /// Vision OCR per-page extraction.
    VisionOcr,
}

impl fmt::Display for ContextType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DocumentExtraction => write!(f, "document_extraction"),
            Self::HealthQuery => write!(f, "health_query"),
            Self::NightBatch => write!(f, "night_batch"),
            Self::VisionOcr => write!(f, "vision_ocr"),
        }
    }
}

/// Complete prompt strategy resolved from context + hardware.
#[derive(Debug, Clone, Serialize)]
pub struct PromptStrategy {
    /// Which prompt template kind to use.
    pub kind: PromptStrategyKind,
    /// LLM temperature (0.0-1.0).
    pub temperature: f32,
    /// Max tokens per LLM call.
    pub max_tokens: i32,
    /// Max retries on parse failure (0 = no retry).
    pub max_retries: u32,
    /// Whether to use streaming (enables StreamGuard).
    pub streaming: bool,
}

// ═══════════════════════════════════════════════════════════
// Strategy resolution
// ═══════════════════════════════════════════════════════════

/// Resolve prompt strategy from calling context and model variant.
///
/// Pure function — no I/O, no side effects.
/// Decision matrix grounded in product decisions + benchmark data.
///
/// For GPU-aware resolution, use `resolve_strategy_with_gpu()`.
pub fn resolve_strategy(context: ContextType, variant: ModelVariant) -> PromptStrategy {
    resolve_strategy_with_gpu(context, variant, false)
}

/// Resolve prompt strategy with explicit GPU flag.
///
/// GPU flag affects DocumentExtraction: Q8/F16 on GPU use MarkdownList
/// (not LegacyJson) because GPU Vulkan has 45% degen on JSON (MF-23).
pub fn resolve_strategy_with_gpu(
    context: ContextType,
    variant: ModelVariant,
    gpu: bool,
) -> PromptStrategy {
    let kind = resolve_kind(context, variant, gpu);
    let max_tokens = resolve_max_tokens(context, kind);
    let max_retries = resolve_max_retries(kind);

    PromptStrategy {
        kind,
        temperature: 0.1, // Accuracy-first: all contexts use 0.1 ([RM-ZE-QJ], BM-04/05/06)
        max_tokens,
        max_retries,
        streaming: true, // Always stream — StreamGuard always active
    }
}

// ═══════════════════════════════════════════════════════════
// Decision matrix
// ═══════════════════════════════════════════════════════════

/// Resolve the prompt strategy kind from the decision matrix.
///
/// STR-01: Context alone determines strategy. Variant and GPU no longer
/// affect strategy choice — both MarkdownList and IterativeDrill have
/// 0% degeneration on all configurations (BM-05/06).
fn resolve_kind(context: ContextType, _variant: ModelVariant, _gpu: bool) -> PromptStrategyKind {
    match context {
        // HealthQuery: MarkdownList — fast, 0% degen, real-time
        ContextType::HealthQuery => PromptStrategyKind::MarkdownList,

        // NightBatch: IterativeDrill — thorough (12/12 lab tests), runs during idle
        ContextType::NightBatch => PromptStrategyKind::IterativeDrill,

        // VisionOcr: IterativeDrill — focused per-field Q&A, 0% degen (BM-06).
        // Orchestrator hardcodes IterativeDrill; session strategy must match.
        ContextType::VisionOcr => PromptStrategyKind::IterativeDrill,

        // DocumentExtraction: MarkdownList — fast, 0% degen on all configs
        ContextType::DocumentExtraction => PromptStrategyKind::MarkdownList,
    }
}

/// Resolve max tokens from strategy kind.
///
/// Strategy kind determines response size, not calling context:
/// - IterativeDrill: focused per-field Q&A → enumerate ~200 tokens, drill 3-8 tokens.
///   1024 is generous. 4096 would let a degenerated drill run 1000× over budget.
/// - MarkdownList: full document/conversation extraction → 4096 needed.
fn resolve_max_tokens(_context: ContextType, kind: PromptStrategyKind) -> i32 {
    match kind {
        PromptStrategyKind::IterativeDrill => 1024,
        PromptStrategyKind::MarkdownList => 4096,
    }
}

/// Resolve max retries from strategy kind.
fn resolve_max_retries(kind: PromptStrategyKind) -> u32 {
    match kind {
        // Element-focused strategies use simple prompts — 1 retry is sufficient
        PromptStrategyKind::MarkdownList => 1,
        PromptStrategyKind::IterativeDrill => 1,
    }
}

/// Detect model variant from model name string.
///
/// Pattern matches on name suffix: `-q4`/`:q4` → Q4, `-f16`/`:f16` → F16,
/// default → Q8.
pub fn detect_model_variant(model_name: &str) -> ModelVariant {
    let lower = model_name.to_lowercase();
    if lower.contains("-q4") || lower.contains(":q4") || lower.contains("_q4") {
        ModelVariant::Q4
    } else if lower.contains("-f16") || lower.contains(":f16") || lower.contains("_f16") {
        ModelVariant::F16
    } else {
        ModelVariant::Q8 // Default assumption
    }
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── HealthQuery: always MarkdownList ──────────────────

    #[test]
    fn health_query_q4_markdown_list() {
        let s = resolve_strategy(ContextType::HealthQuery, ModelVariant::Q4);
        assert_eq!(s.kind, PromptStrategyKind::MarkdownList);
    }

    #[test]
    fn health_query_q8_markdown_list() {
        let s = resolve_strategy(ContextType::HealthQuery, ModelVariant::Q8);
        assert_eq!(s.kind, PromptStrategyKind::MarkdownList);
    }

    #[test]
    fn health_query_f16_markdown_list() {
        let s = resolve_strategy(ContextType::HealthQuery, ModelVariant::F16);
        assert_eq!(s.kind, PromptStrategyKind::MarkdownList);
    }

    // ── NightBatch: always IterativeDrill ────────────────

    #[test]
    fn night_batch_q4_iterative_drill() {
        let s = resolve_strategy(ContextType::NightBatch, ModelVariant::Q4);
        assert_eq!(s.kind, PromptStrategyKind::IterativeDrill);
    }

    #[test]
    fn night_batch_q8_iterative_drill() {
        let s = resolve_strategy(ContextType::NightBatch, ModelVariant::Q8);
        assert_eq!(s.kind, PromptStrategyKind::IterativeDrill);
    }

    // ── DocumentExtraction: always MarkdownList (STR-01) ─

    #[test]
    fn doc_extraction_q4_markdown_list() {
        let s = resolve_strategy(ContextType::DocumentExtraction, ModelVariant::Q4);
        assert_eq!(s.kind, PromptStrategyKind::MarkdownList);
    }

    #[test]
    fn doc_extraction_cpu_q8_markdown_list() {
        // STR-01: CPU Q8 now uses MarkdownList (LegacyJson eliminated)
        let s = resolve_strategy_with_gpu(
            ContextType::DocumentExtraction,
            ModelVariant::Q8,
            false,
        );
        assert_eq!(s.kind, PromptStrategyKind::MarkdownList);
    }

    #[test]
    fn doc_extraction_gpu_q8_markdown_list() {
        let s = resolve_strategy_with_gpu(
            ContextType::DocumentExtraction,
            ModelVariant::Q8,
            true,
        );
        assert_eq!(s.kind, PromptStrategyKind::MarkdownList);
    }

    #[test]
    fn doc_extraction_cpu_f16_markdown_list() {
        // STR-01: CPU F16 now uses MarkdownList (LegacyJson eliminated)
        let s = resolve_strategy_with_gpu(
            ContextType::DocumentExtraction,
            ModelVariant::F16,
            false,
        );
        assert_eq!(s.kind, PromptStrategyKind::MarkdownList);
    }

    #[test]
    fn doc_extraction_all_variants_markdown_list() {
        // STR-01: Context alone determines strategy — variant doesn't matter
        for variant in [ModelVariant::Q4, ModelVariant::Q8, ModelVariant::F16] {
            for gpu in [true, false] {
                let s = resolve_strategy_with_gpu(
                    ContextType::DocumentExtraction,
                    variant,
                    gpu,
                );
                assert_eq!(
                    s.kind,
                    PromptStrategyKind::MarkdownList,
                    "DocExtraction should be MarkdownList for {variant:?} gpu={gpu}",
                );
            }
        }
    }

    // ── VisionOcr: always IterativeDrill ────────────────

    #[test]
    fn vision_ocr_always_iterative_drill() {
        for variant in [ModelVariant::Q4, ModelVariant::Q8, ModelVariant::F16] {
            let s = resolve_strategy(ContextType::VisionOcr, variant);
            assert_eq!(s.kind, PromptStrategyKind::IterativeDrill);
        }
    }

    #[test]
    fn vision_ocr_max_tokens_matches_drill() {
        let s = resolve_strategy(ContextType::VisionOcr, ModelVariant::Q8);
        assert_eq!(s.max_tokens, 1024, "VisionOcr+IterativeDrill should use 1024");
    }

    // ── Temperature: 0.1 for all contexts ───────────────

    #[test]
    fn all_contexts_temperature_is_low() {
        for context in [
            ContextType::DocumentExtraction,
            ContextType::HealthQuery,
            ContextType::NightBatch,
            ContextType::VisionOcr,
        ] {
            let s = resolve_strategy(context, ModelVariant::Q8);
            assert!(
                (s.temperature - 0.1).abs() < f32::EPSILON,
                "Temperature should be 0.1 for {context}, got {}",
                s.temperature,
            );
        }
    }

    // ── Streaming always true ───────────────────────────

    #[test]
    fn streaming_always_true() {
        for context in [
            ContextType::DocumentExtraction,
            ContextType::HealthQuery,
            ContextType::NightBatch,
            ContextType::VisionOcr,
        ] {
            let s = resolve_strategy(context, ModelVariant::Q8);
            assert!(s.streaming, "Streaming should be true for {context}");
        }
    }

    // ── detect_model_variant ────────────────────────────

    #[test]
    fn detect_variant_q4() {
        assert_eq!(detect_model_variant("coheara-medgemma-4b-q4"), ModelVariant::Q4);
        assert_eq!(detect_model_variant("model:q4_k_m"), ModelVariant::Q4);
    }

    #[test]
    fn detect_variant_f16() {
        assert_eq!(detect_model_variant("coheara-medgemma-4b-f16"), ModelVariant::F16);
        assert_eq!(detect_model_variant("model:f16"), ModelVariant::F16);
    }

    #[test]
    fn detect_variant_q8_default() {
        assert_eq!(detect_model_variant("medgemma:latest"), ModelVariant::Q8);
        assert_eq!(detect_model_variant("coheara-medgemma-4b-q8"), ModelVariant::Q8);
        assert_eq!(detect_model_variant("unknown-model"), ModelVariant::Q8);
    }

    // ── Max tokens ──────────────────────────────────────

    #[test]
    fn health_query_has_full_max_tokens() {
        let s = resolve_strategy(ContextType::HealthQuery, ModelVariant::Q8);
        assert_eq!(s.max_tokens, 4096);
    }

    #[test]
    fn night_batch_iterative_has_lower_max_tokens() {
        let s = resolve_strategy(ContextType::NightBatch, ModelVariant::Q8);
        assert_eq!(s.max_tokens, 1024); // Enumerate phase
    }

    #[test]
    fn doc_extraction_has_full_max_tokens() {
        let s = resolve_strategy(ContextType::DocumentExtraction, ModelVariant::Q8);
        assert_eq!(s.max_tokens, 4096);
    }

    // ── Serialization ───────────────────────────────────

    #[test]
    fn context_type_serializes() {
        let json = serde_json::to_string(&ContextType::HealthQuery).unwrap();
        assert_eq!(json, "\"health_query\"");
    }

    #[test]
    fn prompt_strategy_serializes() {
        let s = resolve_strategy(ContextType::HealthQuery, ModelVariant::Q8);
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"markdown_list\""));
        assert!(json.contains("\"streaming\":true"));
    }

    // ── Max tokens driven by kind ───────────────────────

    #[test]
    fn iterative_drill_always_1024() {
        // All contexts resolving to IterativeDrill get 1024
        for context in [ContextType::NightBatch, ContextType::VisionOcr] {
            let s = resolve_strategy(context, ModelVariant::Q8);
            assert_eq!(s.kind, PromptStrategyKind::IterativeDrill);
            assert_eq!(
                s.max_tokens, 1024,
                "IterativeDrill should use 1024 for {context}"
            );
        }
    }

    #[test]
    fn markdown_list_always_4096() {
        // All contexts resolving to MarkdownList get 4096
        for context in [ContextType::HealthQuery, ContextType::DocumentExtraction] {
            let s = resolve_strategy(context, ModelVariant::Q8);
            assert_eq!(s.kind, PromptStrategyKind::MarkdownList);
            assert_eq!(
                s.max_tokens, 4096,
                "MarkdownList should use 4096 for {context}"
            );
        }
    }

    // ── Display ─────────────────────────────────────────

    #[test]
    fn context_type_display() {
        assert_eq!(format!("{}", ContextType::HealthQuery), "health_query");
        assert_eq!(format!("{}", ContextType::NightBatch), "night_batch");
    }
}
