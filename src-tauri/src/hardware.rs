//! Hardware detection for GPU/CPU classification.
//!
//! Queries Ollama `/api/ps` to determine whether models run on GPU or CPU.
//! This drives pipeline configuration (timeouts, context windows, warm strategy).

use serde::{Deserialize, Serialize};

use crate::pipeline::structuring::ollama::OllamaClient;

// ═══════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════

/// GPU availability classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuTier {
    /// All model layers in VRAM.
    FullGpu,
    /// Some layers in VRAM, rest on CPU.
    PartialGpu,
    /// No VRAM allocated — pure CPU inference.
    CpuOnly,
}

impl std::fmt::Display for GpuTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FullGpu => write!(f, "Full GPU"),
            Self::PartialGpu => write!(f, "Partial GPU"),
            Self::CpuOnly => write!(f, "CPU only"),
        }
    }
}

/// Hardware profile detected from Ollama's running models.
///
/// Represents the inference hardware available for the current session.
/// Conservative: defaults to CPU-only if detection fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    /// Whether GPU acceleration is available.
    pub gpu_available: bool,
    /// Total VRAM allocated to loaded models (bytes). 0 = CPU-only.
    pub vram_bytes: u64,
    /// Total model size in memory (bytes).
    pub total_model_bytes: u64,
    /// Processor label from Ollama (e.g., "100% GPU", "CPU").
    pub processor_label: String,
    /// ISO 8601 timestamp when detection occurred.
    pub detected_at: String,
}

impl HardwareProfile {
    /// Classify GPU availability from the profile data.
    pub fn gpu_tier(&self) -> GpuTier {
        if self.total_model_bytes == 0 {
            // No models loaded — can't determine GPU availability
            return GpuTier::CpuOnly;
        }
        if self.vram_bytes == 0 {
            GpuTier::CpuOnly
        } else if self.vram_bytes >= self.total_model_bytes {
            GpuTier::FullGpu
        } else {
            GpuTier::PartialGpu
        }
    }

    /// Conservative fallback when detection fails.
    pub fn cpu_fallback() -> Self {
        Self {
            gpu_available: false,
            vram_bytes: 0,
            total_model_bytes: 0,
            processor_label: "CPU (detection unavailable)".to_string(),
            detected_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

// ═══════════════════════════════════════════════════════════
// Detection
// ═══════════════════════════════════════════════════════════

/// Detect hardware profile by querying Ollama `/api/ps`.
///
/// Falls back to `HardwareProfile::cpu_fallback()` if Ollama is unreachable
/// or no models are currently loaded.
pub fn detect_hardware(client: &OllamaClient) -> HardwareProfile {
    let _span = tracing::info_span!("hardware_detect").entered();

    match client.list_running_models() {
        Ok(models) if !models.is_empty() => {
            let total_size: u64 = models.iter().map(|m| m.size).sum();
            let total_vram: u64 = models.iter().map(|m| m.size_vram).sum();

            // Use the processor label from the first model
            let processor_label = if models.len() == 1 {
                models[0].processor.clone()
            } else {
                // Multiple models — summarize
                let gpu_count = models.iter().filter(|m| m.size_vram > 0).count();
                if gpu_count == models.len() {
                    "GPU (all models)".to_string()
                } else if gpu_count > 0 {
                    format!("Mixed ({gpu_count}/{} on GPU)", models.len())
                } else {
                    "CPU (all models)".to_string()
                }
            };

            let profile = HardwareProfile {
                gpu_available: total_vram > 0,
                vram_bytes: total_vram,
                total_model_bytes: total_size,
                processor_label,
                detected_at: chrono::Utc::now().to_rfc3339(),
            };

            tracing::info!(
                gpu_tier = %profile.gpu_tier(),
                vram_mb = total_vram / 1_000_000,
                total_mb = total_size / 1_000_000,
                models = models.len(),
                "Hardware profile detected"
            );

            profile
        }
        Ok(_) => {
            tracing::info!("No models loaded in Ollama — assuming CPU");
            HardwareProfile::cpu_fallback()
        }
        Err(e) => {
            tracing::warn!(error = %e, "Hardware detection failed — assuming CPU");
            HardwareProfile::cpu_fallback()
        }
    }
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_gpu_when_all_vram() {
        let profile = HardwareProfile {
            gpu_available: true,
            vram_bytes: 5_000_000_000,
            total_model_bytes: 5_000_000_000,
            processor_label: "100% GPU".into(),
            detected_at: "2026-01-01T00:00:00Z".into(),
        };
        assert_eq!(profile.gpu_tier(), GpuTier::FullGpu);
    }

    #[test]
    fn partial_gpu_when_some_vram() {
        let profile = HardwareProfile {
            gpu_available: true,
            vram_bytes: 2_000_000_000,
            total_model_bytes: 5_000_000_000,
            processor_label: "40% GPU".into(),
            detected_at: "2026-01-01T00:00:00Z".into(),
        };
        assert_eq!(profile.gpu_tier(), GpuTier::PartialGpu);
    }

    #[test]
    fn cpu_only_when_no_vram() {
        let profile = HardwareProfile {
            gpu_available: false,
            vram_bytes: 0,
            total_model_bytes: 5_000_000_000,
            processor_label: "CPU".into(),
            detected_at: "2026-01-01T00:00:00Z".into(),
        };
        assert_eq!(profile.gpu_tier(), GpuTier::CpuOnly);
    }

    #[test]
    fn cpu_only_when_no_models_loaded() {
        let profile = HardwareProfile {
            gpu_available: false,
            vram_bytes: 0,
            total_model_bytes: 0,
            processor_label: "Unknown".into(),
            detected_at: "2026-01-01T00:00:00Z".into(),
        };
        assert_eq!(profile.gpu_tier(), GpuTier::CpuOnly);
    }

    #[test]
    fn cpu_fallback_is_conservative() {
        let profile = HardwareProfile::cpu_fallback();
        assert!(!profile.gpu_available);
        assert_eq!(profile.vram_bytes, 0);
        assert_eq!(profile.total_model_bytes, 0);
        assert_eq!(profile.gpu_tier(), GpuTier::CpuOnly);
        assert!(!profile.detected_at.is_empty());
    }

    #[test]
    fn gpu_tier_display() {
        assert_eq!(GpuTier::FullGpu.to_string(), "Full GPU");
        assert_eq!(GpuTier::PartialGpu.to_string(), "Partial GPU");
        assert_eq!(GpuTier::CpuOnly.to_string(), "CPU only");
    }

    #[test]
    fn gpu_tier_serializes_snake_case() {
        let json = serde_json::to_string(&GpuTier::FullGpu).unwrap();
        assert_eq!(json, "\"full_gpu\"");
        let json = serde_json::to_string(&GpuTier::CpuOnly).unwrap();
        assert_eq!(json, "\"cpu_only\"");
    }

    #[test]
    fn hardware_profile_serializes() {
        let profile = HardwareProfile {
            gpu_available: true,
            vram_bytes: 4_000_000_000,
            total_model_bytes: 4_000_000_000,
            processor_label: "100% GPU".into(),
            detected_at: "2026-02-22T12:00:00Z".into(),
        };
        let json = serde_json::to_string(&profile).unwrap();
        assert!(json.contains("\"gpu_available\":true"));
        assert!(json.contains("\"vram_bytes\":4000000000"));
        assert!(json.contains("100% GPU"));
    }

    #[test]
    fn detect_hardware_falls_back_when_unreachable() {
        let client = OllamaClient::new("http://localhost:99999");
        let profile = detect_hardware(&client);
        assert_eq!(profile.gpu_tier(), GpuTier::CpuOnly);
        assert!(!profile.gpu_available);
    }
}
