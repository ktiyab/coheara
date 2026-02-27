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

/// Check if a strategy kind is safe for the given variant + GPU.
///
/// "Safe" = 0% degeneration rate in benchmark data.
pub fn is_strategy_safe(kind: PromptStrategyKind, variant: ModelVariant, gpu: bool) -> bool {
    match kind {
        PromptStrategyKind::MarkdownList => true, // 0% degen on all configs (BM-05)
        PromptStrategyKind::IterativeDrill => {
            // 0% on Q4_K_M+. Only Q4_K_S had 25% (excluded from ModelVariant).
            true
        }
        PromptStrategyKind::LegacyJson => {
            // Safe only on CPU Q8+ (BM-04: 0% degen on CPU Q8, 36% on Q4, 45% on GPU)
            matches!(variant, ModelVariant::Q8 | ModelVariant::F16) && !gpu
        }
    }
}

// ═══════════════════════════════════════════════════════════
// Decision matrix
// ═══════════════════════════════════════════════════════════

/// Resolve the prompt strategy kind from the decision matrix.
fn resolve_kind(context: ContextType, variant: ModelVariant, gpu: bool) -> PromptStrategyKind {
    match context {
        // HealthQuery: always MarkdownList (accurate, 0% degen)
        ContextType::HealthQuery => PromptStrategyKind::MarkdownList,

        // NightBatch: always IterativeDrill (thorough, runs during idle)
        ContextType::NightBatch => PromptStrategyKind::IterativeDrill,

        // VisionOcr: always MarkdownList (simple prompts for vision)
        ContextType::VisionOcr => PromptStrategyKind::MarkdownList,

        // DocumentExtraction: variant + GPU dependent
        ContextType::DocumentExtraction => match variant {
            ModelVariant::Q4 => PromptStrategyKind::MarkdownList, // JSON degens 36% on Q4
            ModelVariant::Q8 | ModelVariant::F16 => {
                if gpu {
                    PromptStrategyKind::MarkdownList // GPU: 45% degen on JSON (MF-23)
                } else {
                    PromptStrategyKind::LegacyJson // CPU Q8+: 0% degen on JSON (BM-04)
                }
            }
        },
    }
}

/// Resolve max tokens from context + strategy kind.
fn resolve_max_tokens(context: ContextType, kind: PromptStrategyKind) -> i32 {
    match context {
        ContextType::HealthQuery => 4096, // Thorough medical research findings
        ContextType::VisionOcr => 4096,
        ContextType::DocumentExtraction => 4096,
        ContextType::NightBatch => match kind {
            PromptStrategyKind::IterativeDrill => 1024, // Enumerate phase
            _ => 4096,
        },
    }
}

/// Resolve max retries from strategy kind.
fn resolve_max_retries(kind: PromptStrategyKind) -> u32 {
    match kind {
        PromptStrategyKind::MarkdownList => 1,    // Simple format, 1 retry enough
        PromptStrategyKind::IterativeDrill => 1,   // Simple Q&A, 1 retry enough
        PromptStrategyKind::LegacyJson => 2,       // Complex format, keep existing 2 retries
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

    // ── DocumentExtraction: variant + GPU dependent ─────

    #[test]
    fn doc_extraction_q4_markdown_list() {
        let s = resolve_strategy(ContextType::DocumentExtraction, ModelVariant::Q4);
        assert_eq!(s.kind, PromptStrategyKind::MarkdownList);
    }

    #[test]
    fn doc_extraction_cpu_q8_legacy_json() {
        // CPU Q8: LegacyJson safe (0% degen in BM-04)
        let s = resolve_strategy_with_gpu(
            ContextType::DocumentExtraction,
            ModelVariant::Q8,
            false,
        );
        assert_eq!(s.kind, PromptStrategyKind::LegacyJson);
    }

    #[test]
    fn doc_extraction_gpu_q8_markdown_list() {
        // GPU Q8: MarkdownList (45% degen on JSON, MF-23)
        let s = resolve_strategy_with_gpu(
            ContextType::DocumentExtraction,
            ModelVariant::Q8,
            true,
        );
        assert_eq!(s.kind, PromptStrategyKind::MarkdownList);
    }

    #[test]
    fn doc_extraction_cpu_f16_legacy_json() {
        let s = resolve_strategy_with_gpu(
            ContextType::DocumentExtraction,
            ModelVariant::F16,
            false,
        );
        assert_eq!(s.kind, PromptStrategyKind::LegacyJson);
    }

    // ── VisionOcr: always MarkdownList ──────────────────

    #[test]
    fn vision_ocr_always_markdown_list() {
        for variant in [ModelVariant::Q4, ModelVariant::Q8, ModelVariant::F16] {
            let s = resolve_strategy(ContextType::VisionOcr, variant);
            assert_eq!(s.kind, PromptStrategyKind::MarkdownList);
        }
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

    // ── Safety checks ───────────────────────────────────

    #[test]
    fn legacy_json_unsafe_on_q4() {
        assert!(!is_strategy_safe(
            PromptStrategyKind::LegacyJson,
            ModelVariant::Q4,
            false,
        ));
    }

    #[test]
    fn legacy_json_unsafe_on_gpu() {
        assert!(!is_strategy_safe(
            PromptStrategyKind::LegacyJson,
            ModelVariant::Q8,
            true,
        ));
    }

    #[test]
    fn legacy_json_safe_on_cpu_q8() {
        assert!(is_strategy_safe(
            PromptStrategyKind::LegacyJson,
            ModelVariant::Q8,
            false,
        ));
    }

    #[test]
    fn markdown_list_always_safe() {
        for variant in [ModelVariant::Q4, ModelVariant::Q8, ModelVariant::F16] {
            for gpu in [true, false] {
                assert!(is_strategy_safe(PromptStrategyKind::MarkdownList, variant, gpu));
            }
        }
    }

    #[test]
    fn iterative_drill_always_safe() {
        // Q4_K_S excluded from ModelVariant, so IterativeDrill safe on all remaining
        for variant in [ModelVariant::Q4, ModelVariant::Q8, ModelVariant::F16] {
            assert!(is_strategy_safe(PromptStrategyKind::IterativeDrill, variant, false));
        }
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

    // ── Display ─────────────────────────────────────────

    #[test]
    fn context_type_display() {
        assert_eq!(format!("{}", ContextType::HealthQuery), "health_query");
        assert_eq!(format!("{}", ContextType::NightBatch), "night_batch");
    }
}
