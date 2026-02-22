//! Time estimation for document processing based on hardware profile.
//!
//! Provides informed progress estimates by combining hardware-detected
//! inference speed with document characteristics (page count).

use serde::Serialize;

use crate::pipeline_config::{PipelineConfig, WarmStrategy};

// ═══════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════

/// Estimated tokens generated per page during vision OCR extraction.
const TOKENS_PER_PAGE_VISION: u32 = 2000;

/// Estimated tokens generated per page during LLM structuring.
const TOKENS_PER_PAGE_STRUCTURING: u32 = 1500;

/// Fixed overhead for pipeline setup, file I/O, DB operations (seconds).
const PIPELINE_OVERHEAD_SECS: u64 = 5;

// ═══════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════

/// Estimated processing time breakdown by stage.
#[derive(Debug, Clone, Serialize)]
pub struct ProcessingEstimate {
    /// Estimated vision extraction time (seconds).
    pub vision_secs: u64,
    /// Model swap time between stages (>0 only for CPU swap strategy).
    pub swap_secs: u64,
    /// Estimated LLM structuring time (seconds).
    pub structuring_secs: u64,
    /// Total estimated processing time (seconds).
    pub total_secs: u64,
}

// ═══════════════════════════════════════════════════════════
// Estimation
// ═══════════════════════════════════════════════════════════

/// Estimate document processing time based on hardware profile and page count.
///
/// Formula per stage: `(tokens_per_page * pages) / tok_per_sec`
/// CPU swap adds `model_load_secs` between stages.
pub fn estimate_processing_time(config: &PipelineConfig, page_count: usize) -> ProcessingEstimate {
    let pages = page_count.max(1) as f32;
    let tok_s = config.estimated_tok_per_sec.max(0.1); // avoid division by zero

    let vision_secs = ((TOKENS_PER_PAGE_VISION as f32 * pages) / tok_s).ceil() as u64;
    let structuring_secs = ((TOKENS_PER_PAGE_STRUCTURING as f32 * pages) / tok_s).ceil() as u64;

    let swap_secs = match config.warm_strategy {
        WarmStrategy::SwapBetweenStages => config.estimated_model_load_secs.ceil() as u64,
        WarmStrategy::WarmBoth => 0,
    };

    let total_secs = vision_secs + swap_secs + structuring_secs + PIPELINE_OVERHEAD_SECS;

    ProcessingEstimate {
        vision_secs,
        swap_secs,
        structuring_secs,
        total_secs,
    }
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu_config() -> PipelineConfig {
        PipelineConfig {
            keep_alive_vision: "30m".into(),
            keep_alive_llm: "30m".into(),
            num_ctx_vision: 2048,
            num_ctx_structuring: 4096,
            warm_strategy: WarmStrategy::WarmBoth,
            estimated_tok_per_sec: 30.0,
            estimated_model_load_secs: 2.0,
        }
    }

    fn cpu_config() -> PipelineConfig {
        PipelineConfig {
            keep_alive_vision: "0".into(),
            keep_alive_llm: "10m".into(),
            num_ctx_vision: 1024,
            num_ctx_structuring: 2048,
            warm_strategy: WarmStrategy::SwapBetweenStages,
            estimated_tok_per_sec: 3.2,
            estimated_model_load_secs: 30.0,
        }
    }

    #[test]
    fn gpu_single_page_fast() {
        let est = estimate_processing_time(&gpu_config(), 1);
        // 2000/30 ≈ 67s vision, 1500/30 = 50s structuring, 0 swap, +5 overhead
        assert_eq!(est.vision_secs, 67);
        assert_eq!(est.structuring_secs, 50);
        assert_eq!(est.swap_secs, 0);
        assert_eq!(est.total_secs, 67 + 50 + 5);
    }

    #[test]
    fn cpu_five_pages_slow() {
        let est = estimate_processing_time(&cpu_config(), 5);
        // 2000*5/3.2 = 3125s vision, 1500*5/3.2 = 2344s structuring, 30s swap, +5 overhead
        assert_eq!(est.vision_secs, 3125);
        assert_eq!(est.structuring_secs, 2344);
        assert_eq!(est.swap_secs, 30);
        assert_eq!(est.total_secs, 3125 + 2344 + 30 + 5);
    }

    #[test]
    fn cpu_has_swap_secs() {
        let est = estimate_processing_time(&cpu_config(), 1);
        assert!(est.swap_secs > 0, "CPU config should have swap time");
    }

    #[test]
    fn gpu_has_no_swap_secs() {
        let est = estimate_processing_time(&gpu_config(), 1);
        assert_eq!(est.swap_secs, 0, "GPU config should have no swap time");
    }

    #[test]
    fn zero_pages_treated_as_one() {
        let est = estimate_processing_time(&gpu_config(), 0);
        assert!(est.total_secs > PIPELINE_OVERHEAD_SECS);
    }

    #[test]
    fn estimate_serializes() {
        let est = estimate_processing_time(&gpu_config(), 2);
        let json = serde_json::to_string(&est).unwrap();
        assert!(json.contains("\"vision_secs\""));
        assert!(json.contains("\"total_secs\""));
    }
}
