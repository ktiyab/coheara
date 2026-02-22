//! Pipeline configuration derived from hardware profile.
//!
//! Maps GPU tier to concrete values: keep_alive durations, context window sizes,
//! model warm-up strategy, and estimated inference speed. These drive the entire
//! document processing pipeline behavior.

use serde::Serialize;

use crate::hardware::{GpuTier, HardwareProfile};

// ═══════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════

/// Model warm-up strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WarmStrategy {
    /// GPU: warm both vision and LLM models simultaneously.
    /// VRAM is sufficient to hold both.
    WarmBoth,
    /// CPU: warm first stage only, then unload and swap between stages.
    /// Prevents OOM on systems with limited RAM.
    SwapBetweenStages,
}

/// Pipeline configuration derived from hardware detection.
///
/// All values are tuned per GPU tier based on benchmarked performance:
/// - FullGpu: 30 tok/s, 2s model load, both models fit in VRAM
/// - PartialGpu: 15 tok/s, 5s model load, partial offloading
/// - CpuOnly: 3.2 tok/s (benchmarked MedGemma), 30s model load
#[derive(Debug, Clone, Serialize)]
pub struct PipelineConfig {
    /// Ollama `keep_alive` for vision model.
    /// CPU: "0" (unload after extraction to free RAM for structuring).
    pub keep_alive_vision: String,
    /// Ollama `keep_alive` for LLM structuring model.
    pub keep_alive_llm: String,
    /// Context window for vision extraction (per-page, smaller = less KV cache).
    pub num_ctx_vision: u32,
    /// Context window for LLM structuring (full document text, larger).
    pub num_ctx_structuring: u32,
    /// Model warm-up strategy.
    pub warm_strategy: WarmStrategy,
    /// Estimated tokens per second for time calculations.
    pub estimated_tok_per_sec: f32,
    /// Estimated model loading time in seconds.
    pub estimated_model_load_secs: f32,
}

// ═══════════════════════════════════════════════════════════
// Derivation
// ═══════════════════════════════════════════════════════════

/// Derive pipeline config from a hardware profile.
///
/// Values are based on MEDGEMMA-BENCHMARK-02 results and Ollama documentation.
pub fn derive_config(profile: &HardwareProfile) -> PipelineConfig {
    match profile.gpu_tier() {
        GpuTier::FullGpu => PipelineConfig {
            keep_alive_vision: "30m".into(),
            keep_alive_llm: "30m".into(),
            num_ctx_vision: 2048,
            num_ctx_structuring: 4096,
            warm_strategy: WarmStrategy::WarmBoth,
            estimated_tok_per_sec: 30.0,
            estimated_model_load_secs: 2.0,
        },
        GpuTier::PartialGpu => PipelineConfig {
            keep_alive_vision: "15m".into(),
            keep_alive_llm: "15m".into(),
            num_ctx_vision: 1536,
            num_ctx_structuring: 2048,
            warm_strategy: WarmStrategy::WarmBoth,
            estimated_tok_per_sec: 15.0,
            estimated_model_load_secs: 5.0,
        },
        GpuTier::CpuOnly => PipelineConfig {
            keep_alive_vision: "0".into(),
            keep_alive_llm: "10m".into(),
            num_ctx_vision: 1024,
            num_ctx_structuring: 2048,
            warm_strategy: WarmStrategy::SwapBetweenStages,
            estimated_tok_per_sec: 3.2,
            estimated_model_load_secs: 30.0,
        },
    }
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_profile(tier: GpuTier) -> HardwareProfile {
        match tier {
            GpuTier::FullGpu => HardwareProfile {
                gpu_available: true,
                vram_bytes: 5_000_000_000,
                total_model_bytes: 5_000_000_000,
                processor_label: "100% GPU".into(),
                detected_at: "2026-01-01T00:00:00Z".into(),
            },
            GpuTier::PartialGpu => HardwareProfile {
                gpu_available: true,
                vram_bytes: 2_000_000_000,
                total_model_bytes: 5_000_000_000,
                processor_label: "40% GPU".into(),
                detected_at: "2026-01-01T00:00:00Z".into(),
            },
            GpuTier::CpuOnly => HardwareProfile {
                gpu_available: false,
                vram_bytes: 0,
                total_model_bytes: 5_000_000_000,
                processor_label: "CPU".into(),
                detected_at: "2026-01-01T00:00:00Z".into(),
            },
        }
    }

    #[test]
    fn full_gpu_config() {
        let config = derive_config(&make_profile(GpuTier::FullGpu));
        assert_eq!(config.keep_alive_vision, "30m");
        assert_eq!(config.keep_alive_llm, "30m");
        assert_eq!(config.num_ctx_vision, 2048);
        assert_eq!(config.num_ctx_structuring, 4096);
        assert_eq!(config.warm_strategy, WarmStrategy::WarmBoth);
        assert!((config.estimated_tok_per_sec - 30.0).abs() < f32::EPSILON);
    }

    #[test]
    fn partial_gpu_config() {
        let config = derive_config(&make_profile(GpuTier::PartialGpu));
        assert_eq!(config.keep_alive_vision, "15m");
        assert_eq!(config.num_ctx_vision, 1536);
        assert_eq!(config.num_ctx_structuring, 2048);
        assert_eq!(config.warm_strategy, WarmStrategy::WarmBoth);
        assert!((config.estimated_tok_per_sec - 15.0).abs() < f32::EPSILON);
    }

    #[test]
    fn cpu_only_config() {
        let config = derive_config(&make_profile(GpuTier::CpuOnly));
        assert_eq!(config.keep_alive_vision, "0");
        assert_eq!(config.keep_alive_llm, "10m");
        assert_eq!(config.num_ctx_vision, 1024);
        assert_eq!(config.num_ctx_structuring, 2048);
        assert_eq!(config.warm_strategy, WarmStrategy::SwapBetweenStages);
        assert!((config.estimated_tok_per_sec - 3.2).abs() < f32::EPSILON);
        assert!((config.estimated_model_load_secs - 30.0).abs() < f32::EPSILON);
    }

    #[test]
    fn cpu_fallback_produces_cpu_config() {
        let config = derive_config(&HardwareProfile::cpu_fallback());
        assert_eq!(config.warm_strategy, WarmStrategy::SwapBetweenStages);
        assert_eq!(config.keep_alive_vision, "0");
    }

    #[test]
    fn warm_strategy_serializes() {
        let json = serde_json::to_string(&WarmStrategy::WarmBoth).unwrap();
        assert_eq!(json, "\"warm_both\"");
        let json = serde_json::to_string(&WarmStrategy::SwapBetweenStages).unwrap();
        assert_eq!(json, "\"swap_between_stages\"");
    }

    #[test]
    fn pipeline_config_serializes() {
        let config = derive_config(&make_profile(GpuTier::FullGpu));
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"keep_alive_vision\":\"30m\""));
        assert!(json.contains("\"warm_both\""));
    }
}
