//! L6-07: HardwareAdvisor — variant recommendation from hardware profile.
//!
//! Pure-function module that maps detected hardware (RAM, VRAM, GPU tier)
//! to a recommended MedGemma quantization variant (Q4/Q8/F16).
//!
//! Evidence: MF-37 (prompt complexity), MF-44 (Q4_K_M safe minimum),
//! MF-49 (Q4_K_S CPU degen), BM-04/05/06 (degeneration by variant).
//!
//! Extends `hardware.rs` — does not replace it.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::hardware::HardwareProfile;

// ═══════════════════════════════════════════════════════════
// Constants — model sizes and RAM thresholds
// ═══════════════════════════════════════════════════════════

/// Q4_K_M model file size (bytes).
const Q4_SIZE_BYTES: u64 = 3_300_000_000;
/// Q8_0 model file size (bytes).
const Q8_SIZE_BYTES: u64 = 5_000_000_000;
/// F16 model file size (bytes).
const F16_SIZE_BYTES: u64 = 8_600_000_000;

/// Minimum total RAM to recommend Q4 (bytes). ~1.5x headroom.
const MIN_RAM_Q4: u64 = 6_000_000_000;
/// Minimum total RAM to recommend Q8 (bytes).
const MIN_RAM_Q8: u64 = 8_000_000_000;
/// Minimum total RAM to recommend F16 (bytes).
const MIN_RAM_F16: u64 = 14_000_000_000;

/// Minimum VRAM to recommend Q8 on GPU (bytes).
const MIN_VRAM_Q8: u64 = 5_000_000_000;
/// Minimum VRAM to recommend F16 on GPU (bytes).
const MIN_VRAM_F16: u64 = 10_000_000_000;

/// Fallback RAM when system detection fails (8 GB — conservative).
const DEFAULT_RAM_FALLBACK: u64 = 8_000_000_000;

// ═══════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════

/// MedGemma quantization variant.
///
/// Q4_K_S intentionally excluded — 25% degeneration on iterative drill (BM-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelVariant {
    /// Q4_K_M — 3.3 GB, minimum viable quantization.
    Q4,
    /// Q8_0 — 5.0 GB, balanced. Recommended default.
    Q8,
    /// F16 — 8.6 GB, full precision.
    F16,
}

impl ModelVariant {
    /// Ollama model name for this variant.
    pub fn model_name(&self) -> &'static str {
        match self {
            Self::Q4 => "coheara-medgemma-4b-q4",
            Self::Q8 => "coheara-medgemma-4b-q8",
            Self::F16 => "coheara-medgemma-4b-f16",
        }
    }

    /// Model file size in bytes.
    pub fn file_size_bytes(&self) -> u64 {
        match self {
            Self::Q4 => Q4_SIZE_BYTES,
            Self::Q8 => Q8_SIZE_BYTES,
            Self::F16 => F16_SIZE_BYTES,
        }
    }

    /// Human-readable quantization label.
    pub fn quantization_label(&self) -> &'static str {
        match self {
            Self::Q4 => "Q4_K_M",
            Self::Q8 => "Q8_0",
            Self::F16 => "F16",
        }
    }
}

impl fmt::Display for ModelVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.model_name(), self.quantization_label())
    }
}

/// System resource snapshot for variant recommendation.
#[derive(Debug, Clone)]
pub struct SystemResources {
    /// Total system RAM in bytes.
    pub total_ram_bytes: u64,
    /// Hardware profile from Ollama (None if no models loaded yet).
    pub hardware_profile: Option<HardwareProfile>,
}

/// Structured variant recommendation with reasoning.
#[derive(Debug, Clone, Serialize)]
pub struct VariantRecommendation {
    /// Recommended quantization variant.
    pub variant: ModelVariant,
    /// Ollama model name (e.g., "coheara-medgemma-4b-q8").
    pub model_name: String,
    /// Model file size in bytes (for download UI).
    pub model_size_bytes: u64,
    /// Human-readable recommendation reason.
    pub reason: String,
    /// Whether GPU inference is reliable for this hardware.
    pub gpu_reliable: bool,
    /// Next-tier variant if user gets more resources.
    pub can_upgrade: Option<ModelVariant>,
    /// Warnings (e.g., GPU reliability issues).
    pub warnings: Vec<String>,
}

// ═══════════════════════════════════════════════════════════
// Recommendation logic
// ═══════════════════════════════════════════════════════════

/// Recommend a model variant based on system resources.
///
/// Pure function — no I/O, no side effects.
/// Decision matrix grounded in BM-04/05/06 benchmark data.
pub fn recommend_variant(resources: &SystemResources) -> VariantRecommendation {
    let mut warnings: Vec<String> = Vec::new();
    let mut gpu_reliable = true;

    // Check for known-unreliable GPU
    if let Some(ref profile) = resources.hardware_profile {
        if is_unreliable_gpu(&profile.processor_label) {
            gpu_reliable = false;
            warnings.push(
                "GPU (Vulkan gfx1010) has 45% degeneration rate on JSON extraction. \
                 Use simplified prompts or CPU inference."
                    .to_string(),
            );
        }
    }

    // GPU-based recommendation (preferred when reliable GPU available)
    if let Some(ref profile) = resources.hardware_profile {
        if profile.gpu_available && profile.vram_bytes > 0 && gpu_reliable {
            return recommend_from_vram(profile.vram_bytes, resources.total_ram_bytes, warnings);
        }
    }

    // CPU/RAM-based recommendation
    recommend_from_ram(resources.total_ram_bytes, gpu_reliable, warnings)
}

/// Recommend based on available VRAM (GPU path).
fn recommend_from_vram(
    vram_bytes: u64,
    total_ram_bytes: u64,
    mut warnings: Vec<String>,
) -> VariantRecommendation {
    if vram_bytes >= MIN_VRAM_F16 {
        let can_upgrade = None; // Already at max
        VariantRecommendation {
            variant: ModelVariant::F16,
            model_name: ModelVariant::F16.model_name().to_string(),
            model_size_bytes: ModelVariant::F16.file_size_bytes(),
            reason: format!(
                "{:.1} GB VRAM available — full precision fits comfortably",
                vram_bytes as f64 / 1e9
            ),
            gpu_reliable: true,
            can_upgrade,
            warnings,
        }
    } else if vram_bytes >= MIN_VRAM_Q8 {
        let can_upgrade = if total_ram_bytes >= MIN_RAM_F16 || vram_bytes >= MIN_VRAM_F16 {
            Some(ModelVariant::F16)
        } else {
            None
        };
        VariantRecommendation {
            variant: ModelVariant::Q8,
            model_name: ModelVariant::Q8.model_name().to_string(),
            model_size_bytes: ModelVariant::Q8.file_size_bytes(),
            reason: format!(
                "{:.1} GB VRAM available — Q8 recommended for balanced performance",
                vram_bytes as f64 / 1e9
            ),
            gpu_reliable: true,
            can_upgrade,
            warnings,
        }
    } else {
        let can_upgrade = if total_ram_bytes >= MIN_RAM_Q8 {
            Some(ModelVariant::Q8)
        } else {
            None
        };
        if vram_bytes < MIN_VRAM_Q8 {
            warnings.push(format!(
                "GPU VRAM ({:.1} GB) insufficient for Q8. Consider CPU inference.",
                vram_bytes as f64 / 1e9
            ));
        }
        VariantRecommendation {
            variant: ModelVariant::Q4,
            model_name: ModelVariant::Q4.model_name().to_string(),
            model_size_bytes: ModelVariant::Q4.file_size_bytes(),
            reason: format!(
                "{:.1} GB VRAM available — Q4 fits within GPU memory",
                vram_bytes as f64 / 1e9
            ),
            gpu_reliable: true,
            can_upgrade,
            warnings,
        }
    }
}

/// Recommend based on total system RAM (CPU path).
fn recommend_from_ram(
    total_ram_bytes: u64,
    gpu_reliable: bool,
    warnings: Vec<String>,
) -> VariantRecommendation {
    if total_ram_bytes >= MIN_RAM_F16 {
        VariantRecommendation {
            variant: ModelVariant::F16,
            model_name: ModelVariant::F16.model_name().to_string(),
            model_size_bytes: ModelVariant::F16.file_size_bytes(),
            reason: format!(
                "{:.0} GB RAM available — full precision recommended",
                total_ram_bytes as f64 / 1e9
            ),
            gpu_reliable,
            can_upgrade: None,
            warnings,
        }
    } else if total_ram_bytes >= MIN_RAM_Q8 {
        let can_upgrade = if total_ram_bytes >= MIN_RAM_F16 {
            Some(ModelVariant::F16)
        } else {
            None
        };
        VariantRecommendation {
            variant: ModelVariant::Q8,
            model_name: ModelVariant::Q8.model_name().to_string(),
            model_size_bytes: ModelVariant::Q8.file_size_bytes(),
            reason: format!(
                "{:.0} GB RAM available — Q8 recommended (0% degeneration on CPU)",
                total_ram_bytes as f64 / 1e9
            ),
            gpu_reliable,
            can_upgrade,
            warnings,
        }
    } else if total_ram_bytes >= MIN_RAM_Q4 {
        VariantRecommendation {
            variant: ModelVariant::Q4,
            model_name: ModelVariant::Q4.model_name().to_string(),
            model_size_bytes: ModelVariant::Q4.file_size_bytes(),
            reason: format!(
                "{:.0} GB RAM available — Q4 is the largest variant that fits",
                total_ram_bytes as f64 / 1e9
            ),
            gpu_reliable,
            can_upgrade: Some(ModelVariant::Q8),
            warnings,
        }
    } else {
        // Below 6 GB — still recommend Q4 as minimum viable
        let mut warnings = warnings;
        warnings.push(format!(
            "System RAM ({:.1} GB) is below recommended minimum (6 GB). \
             Performance may be degraded.",
            total_ram_bytes as f64 / 1e9
        ));
        VariantRecommendation {
            variant: ModelVariant::Q4,
            model_name: ModelVariant::Q4.model_name().to_string(),
            model_size_bytes: ModelVariant::Q4.file_size_bytes(),
            reason: "Q4 is the smallest supported variant".to_string(),
            gpu_reliable,
            can_upgrade: Some(ModelVariant::Q8),
            warnings,
        }
    }
}

/// Check if GPU processor label indicates an unreliable configuration.
///
/// Known: Vulkan gfx1010 (AMD RX 5700 XT) has 45% degeneration on
/// JSON extraction (BM-04 MF-23).
fn is_unreliable_gpu(processor_label: &str) -> bool {
    let label = processor_label.to_lowercase();
    label.contains("gfx1010") || label.contains("gfx1011")
}

// ═══════════════════════════════════════════════════════════
// System RAM detection
// ═══════════════════════════════════════════════════════════

/// Detect total system RAM in bytes.
///
/// Reads `/proc/meminfo` on Linux. Falls back to `DEFAULT_RAM_FALLBACK` (8 GB)
/// if detection fails (e.g., non-Linux platform or permission issue).
pub fn detect_system_ram() -> u64 {
    detect_system_ram_inner().unwrap_or(DEFAULT_RAM_FALLBACK)
}

/// Inner implementation — returns None on failure.
fn detect_system_ram_inner() -> Option<u64> {
    let contents = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in contents.lines() {
        if line.starts_with("MemTotal:") {
            // Format: "MemTotal:       16384000 kB"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let kb: u64 = parts[1].parse().ok()?;
                return Some(kb * 1024); // Convert kB to bytes
            }
        }
    }
    None
}

/// Build SystemResources from optional HardwareProfile + detected RAM.
pub fn build_system_resources(profile: Option<HardwareProfile>) -> SystemResources {
    SystemResources {
        total_ram_bytes: detect_system_ram(),
        hardware_profile: profile,
    }
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::HardwareProfile;

    fn gb(n: u64) -> u64 {
        n * 1_000_000_000
    }

    fn cpu_resources(ram_gb: u64) -> SystemResources {
        SystemResources {
            total_ram_bytes: gb(ram_gb),
            hardware_profile: None,
        }
    }

    fn gpu_resources(ram_gb: u64, vram_gb: u64, processor: &str) -> SystemResources {
        SystemResources {
            total_ram_bytes: gb(ram_gb),
            hardware_profile: Some(HardwareProfile {
                gpu_available: true,
                vram_bytes: gb(vram_gb),
                total_model_bytes: gb(vram_gb),
                processor_label: processor.to_string(),
                detected_at: "2026-02-26T00:00:00Z".to_string(),
            }),
        }
    }

    // ── RAM-based (no GPU) ───────────────────────────────

    #[test]
    fn low_ram_recommends_q4() {
        let rec = recommend_variant(&cpu_resources(4));
        assert_eq!(rec.variant, ModelVariant::Q4);
        assert!(rec.warnings.iter().any(|w| w.contains("below recommended")));
    }

    #[test]
    fn boundary_6gb_recommends_q4() {
        let rec = recommend_variant(&cpu_resources(6));
        assert_eq!(rec.variant, ModelVariant::Q4);
        assert_eq!(rec.can_upgrade, Some(ModelVariant::Q8));
    }

    #[test]
    fn boundary_8gb_recommends_q8() {
        let rec = recommend_variant(&cpu_resources(8));
        assert_eq!(rec.variant, ModelVariant::Q8);
    }

    #[test]
    fn medium_ram_recommends_q8() {
        let rec = recommend_variant(&cpu_resources(10));
        assert_eq!(rec.variant, ModelVariant::Q8);
        assert!(rec.reason.contains("Q8"));
    }

    #[test]
    fn boundary_14gb_recommends_f16() {
        let rec = recommend_variant(&cpu_resources(14));
        assert_eq!(rec.variant, ModelVariant::F16);
    }

    #[test]
    fn high_ram_recommends_f16() {
        let rec = recommend_variant(&cpu_resources(32));
        assert_eq!(rec.variant, ModelVariant::F16);
        assert!(rec.reason.contains("full precision"));
    }

    // ── GPU VRAM-based ───────────────────────────────────

    #[test]
    fn gpu_vram_low_recommends_q4() {
        let rec = recommend_variant(&gpu_resources(16, 4, "100% GPU"));
        assert_eq!(rec.variant, ModelVariant::Q4);
    }

    #[test]
    fn gpu_vram_mid_recommends_q8() {
        let rec = recommend_variant(&gpu_resources(16, 6, "100% GPU"));
        assert_eq!(rec.variant, ModelVariant::Q8);
    }

    #[test]
    fn gpu_vram_high_recommends_f16() {
        let rec = recommend_variant(&gpu_resources(16, 12, "100% GPU"));
        assert_eq!(rec.variant, ModelVariant::F16);
    }

    // ── Unreliable GPU ───────────────────────────────────

    #[test]
    fn gfx1010_adds_warning_and_falls_to_ram() {
        let rec = recommend_variant(&gpu_resources(16, 8, "Vulkan gfx1010"));
        // Should fall back to RAM-based (not use GPU)
        assert!(!rec.gpu_reliable);
        assert!(rec.warnings.iter().any(|w| w.contains("gfx1010")));
        // With 16 GB RAM, should recommend F16 via RAM path
        assert_eq!(rec.variant, ModelVariant::F16);
    }

    #[test]
    fn gfx1010_low_ram_recommends_q8() {
        let rec = recommend_variant(&gpu_resources(8, 8, "Vulkan gfx1010"));
        assert!(!rec.gpu_reliable);
        assert_eq!(rec.variant, ModelVariant::Q8);
    }

    // ── Upgrade path ─────────────────────────────────────

    #[test]
    fn q4_can_upgrade_to_q8() {
        let rec = recommend_variant(&cpu_resources(6));
        assert_eq!(rec.variant, ModelVariant::Q4);
        assert_eq!(rec.can_upgrade, Some(ModelVariant::Q8));
    }

    #[test]
    fn f16_has_no_upgrade() {
        let rec = recommend_variant(&cpu_resources(32));
        assert_eq!(rec.variant, ModelVariant::F16);
        assert_eq!(rec.can_upgrade, None);
    }

    // ── Model name format ────────────────────────────────

    #[test]
    fn model_names_correct() {
        assert_eq!(ModelVariant::Q4.model_name(), "coheara-medgemma-4b-q4");
        assert_eq!(ModelVariant::Q8.model_name(), "coheara-medgemma-4b-q8");
        assert_eq!(ModelVariant::F16.model_name(), "coheara-medgemma-4b-f16");
    }

    #[test]
    fn file_sizes_correct() {
        assert_eq!(ModelVariant::Q4.file_size_bytes(), 3_300_000_000);
        assert_eq!(ModelVariant::Q8.file_size_bytes(), 5_000_000_000);
        assert_eq!(ModelVariant::F16.file_size_bytes(), 8_600_000_000);
    }

    #[test]
    fn quantization_labels_correct() {
        assert_eq!(ModelVariant::Q4.quantization_label(), "Q4_K_M");
        assert_eq!(ModelVariant::Q8.quantization_label(), "Q8_0");
        assert_eq!(ModelVariant::F16.quantization_label(), "F16");
    }

    // ── Display ──────────────────────────────────────────

    #[test]
    fn variant_display() {
        let s = format!("{}", ModelVariant::Q8);
        assert!(s.contains("coheara-medgemma-4b-q8"));
        assert!(s.contains("Q8_0"));
    }

    // ── Serialization ────────────────────────────────────

    #[test]
    fn variant_serializes_snake_case() {
        let json = serde_json::to_string(&ModelVariant::Q4).unwrap();
        assert_eq!(json, "\"q4\"");
        let json = serde_json::to_string(&ModelVariant::Q8).unwrap();
        assert_eq!(json, "\"q8\"");
    }

    #[test]
    fn recommendation_serializes() {
        let rec = recommend_variant(&cpu_resources(8));
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.contains("\"variant\":\"q8\""));
        assert!(json.contains("\"model_name\":\"coheara-medgemma-4b-q8\""));
    }

    // ── System RAM detection ─────────────────────────────

    #[test]
    fn detect_system_ram_returns_nonzero() {
        let ram = detect_system_ram();
        assert!(ram > 0);
    }

    // ── No profile uses RAM only ─────────────────────────

    #[test]
    fn no_hardware_profile_uses_ram_only() {
        let resources = SystemResources {
            total_ram_bytes: gb(12),
            hardware_profile: None,
        };
        let rec = recommend_variant(&resources);
        // 12 GB RAM, no GPU → Q8
        assert_eq!(rec.variant, ModelVariant::Q8);
        assert!(rec.gpu_reliable);
    }
}
