//! L6-01: Ollama integration types, error taxonomy, and security validators.
//!
//! These types formalize the Ollama HTTP API contract and provide
//! the foundation for all L6 AI Engine operations.

use serde::{Deserialize, Serialize};

// ──────────────────────────────────────────────
// Data Types (Ollama API responses)
// ──────────────────────────────────────────────

/// Enriched model information from Ollama `/api/tags`.
///
/// Extends the previous `OllamaModel { name }` with size, digest,
/// and model family details needed by L6-02 Model Management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    /// Bytes on disk.
    pub size: u64,
    /// SHA-256 digest.
    pub digest: String,
    /// ISO 8601 timestamp.
    pub modified_at: String,
    /// Model family details (may be absent for some models).
    #[serde(default)]
    pub details: ModelDetails,
}

/// Model family metadata from Ollama `/api/tags` details field.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelDetails {
    /// Model family: "gemma", "llama", "phi", etc.
    pub family: Option<String>,
    /// Parameter count string: "4B", "7B", "27B".
    pub parameter_size: Option<String>,
    /// Quantization level: "Q4_K_M", "F16", etc.
    pub quantization_level: Option<String>,
}

// ──────────────────────────────────────────────
// CT-01: Model capability tags
// ──────────────────────────────────────────────

/// User-assigned capability tag for pipeline routing.
///
/// CT-01: Tags drive the extraction path — if a model only accepts TXT,
/// the pipeline converts PDF→text before sending. These are routing
/// instructions, not just metadata.
///
/// Stored in `model_capability_tags` table (migration 017).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum CapabilityTag {
    /// Can process image inputs (multimodal).
    Vision,
    /// Medical domain knowledge.
    Medical,
    /// Native PDF understanding (future).
    Pdf,
    /// Can process PNG images.
    Png,
    /// Can process JPEG images.
    Jpeg,
    /// Can process TIFF images.
    Tiff,
    /// Can process plain text input.
    Txt,
}

impl CapabilityTag {
    /// All valid tags (for enumeration and validation).
    pub fn all() -> &'static [CapabilityTag] {
        &[
            Self::Vision,
            Self::Medical,
            Self::Pdf,
            Self::Png,
            Self::Jpeg,
            Self::Tiff,
            Self::Txt,
        ]
    }

    /// Database string representation (matches CHECK constraint).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Vision => "VISION",
            Self::Medical => "MEDICAL",
            Self::Pdf => "PDF",
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Tiff => "TIFF",
            Self::Txt => "TXT",
        }
    }

    /// Whether this tag indicates vision/image processing capability.
    pub fn is_vision_related(&self) -> bool {
        matches!(self, Self::Vision | Self::Png | Self::Jpeg | Self::Tiff)
    }
}

impl std::fmt::Display for CapabilityTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for CapabilityTag {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "VISION" => Ok(Self::Vision),
            "MEDICAL" => Ok(Self::Medical),
            "PDF" => Ok(Self::Pdf),
            "PNG" => Ok(Self::Png),
            "JPEG" => Ok(Self::Jpeg),
            "TIFF" => Ok(Self::Tiff),
            "TXT" => Ok(Self::Txt),
            _ => Err(format!("Unknown capability tag: '{s}'")),
        }
    }
}

// ──────────────────────────────────────────────
// R3: Vision model types
// ──────────────────────────────────────────────

/// Model capability detected via `/api/show` metadata.
///
/// R3: Used to determine if a model can handle image inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelCapability {
    /// Text-only model (e.g., llama3, mistral).
    TextOnly,
    /// Vision-capable model (e.g., MedGemma multimodal).
    Vision,
}

/// Request body for vision-enabled generation via Ollama `/api/generate`.
///
/// R3: Extends the standard generate request with base64-encoded images.
#[derive(Debug, Clone, Serialize)]
pub struct VisionGenerateRequest {
    pub model: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// Base64-encoded images (PNG or JPEG).
    pub images: Vec<String>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<VisionGenerationOptions>,
    /// Force model unload after request (SEC-02-G09).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_alive: Option<String>,
}

/// Generation options tuned for vision OCR (deterministic extraction).
#[derive(Debug, Clone, Serialize)]
pub struct VisionGenerationOptions {
    /// 0.0 for deterministic document extraction.
    pub temperature: f32,
    /// Maximum tokens for vision extraction.
    pub num_predict: i32,
    /// Context window size (hardware-tiered). None = model default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_ctx: Option<u32>,
}

/// R3: Chat-based vision request for `/api/chat`.
///
/// Required by chat-template models (MedGemma, LLaVA, Gemma) that expect
/// messages-based format. The generate endpoint returns 500 for these models
/// when images are provided.
#[derive(Debug, Clone, Serialize)]
pub struct VisionChatRequest {
    pub model: String,
    pub messages: Vec<VisionChatMessage>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<VisionGenerationOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_alive: Option<String>,
}

/// A single message in a vision chat request.
#[derive(Debug, Clone, Serialize)]
pub struct VisionChatMessage {
    pub role: String,
    pub content: String,
    /// Base64-encoded images (only for user messages).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
}

/// Known vision-capable model name prefixes.
///
/// Used by `detect_capability()` as a fast heuristic before `/api/show`.
pub const VISION_MODEL_PREFIXES: &[&str] = &[
    "llava",
    "moondream",
    "cogvlm",
    "bakllava",
    "medgemma",  // MedGemma 1.5 is multimodal
];

/// Check if a model is vision-capable using the name prefix heuristic.
///
/// R3: Pure function — no network call. Uses `extract_model_component()`
/// to strip namespace before matching against `VISION_MODEL_PREFIXES`.
pub fn is_vision_model(model_name: &str) -> bool {
    let component = extract_model_component(model_name);
    VISION_MODEL_PREFIXES
        .iter()
        .any(|prefix| component.starts_with(prefix))
}

// ──────────────────────────────────────────────
// Health & Operations types
// ──────────────────────────────────────────────

/// Result of a lightweight health check (GET `/`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaHealth {
    pub reachable: bool,
    pub version: Option<String>,
    pub models_count: usize,
}

/// A model currently loaded in Ollama's memory (from `/api/ps`).
///
/// Used by hardware detection to determine GPU vs CPU allocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningModelInfo {
    /// Model name (e.g., "medgemma:4b").
    pub name: String,
    /// Total model size in memory (bytes).
    pub size: u64,
    /// Size loaded into VRAM (bytes). 0 = CPU-only.
    pub size_vram: u64,
    /// Processor label from Ollama (e.g., "100% GPU", "CPU").
    pub processor: String,
}

/// Progress event from model pull (NDJSON from POST `/api/pull`).
///
/// Each line of the streaming response is one `PullProgress`.
/// The frontend receives these as Tauri events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullProgress {
    /// Status string: "pulling manifest", "downloading digestname",
    /// "verifying sha256 digest", "writing manifest", "success".
    pub status: String,
    /// Layer digest being processed (present during download).
    pub digest: Option<String>,
    /// Total bytes for the current layer.
    pub total: Option<u64>,
    /// Completed bytes for the current layer.
    pub completed: Option<u64>,
}

/// Extended model detail from POST `/api/show`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDetail {
    /// Model name (e.g., "medgemma:4b").
    pub name: String,
    /// The Modelfile content.
    pub modelfile: Option<String>,
    /// Model parameters.
    pub parameters: Option<String>,
    /// Template string.
    pub template: Option<String>,
    /// Model details (family, params, quant).
    #[serde(default)]
    pub details: ModelDetails,
}

/// Model preference input for `resolve_model()`.
///
/// This struct comes from L6-04 Model Preferences but is defined
/// here because `resolve_model()` lives on `OllamaClient`.
#[derive(Debug, Clone, Default)]
pub struct ModelPreference {
    /// Explicit user choice (highest priority).
    pub user_selected: Option<String>,
    /// Curated medical model names (fallback).
    pub recommended: Vec<String>,
    /// If true, accept any installed model as last resort.
    pub fallback_any: bool,
}

/// Generation parameters for Ollama `/api/generate`.
///
/// Controls LLM output determinism and quality.
/// R-MOD-04 D3: Medical prompts need low temperature for deterministic output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationOptions {
    /// Sampling temperature (0.0-1.0). Lower = more deterministic.
    /// Medical default: 0.1 for reproducible extractions.
    pub temperature: f32,
    /// Top-p (nucleus) sampling threshold.
    pub top_p: f32,
    /// Top-k sampling: number of top tokens to consider.
    pub top_k: u32,
    /// Maximum tokens in the generated response.
    /// None = model default (typically 2048).
    pub num_predict: Option<i32>,
    /// Context window size (hardware-tiered). None = model default.
    pub num_ctx: Option<u32>,
}

impl Default for GenerationOptions {
    /// Medical-grade defaults: low temperature for deterministic extraction.
    fn default() -> Self {
        Self {
            temperature: 0.1,
            top_p: 0.9,
            top_k: 40,
            num_predict: None,
            num_ctx: None,
        }
    }
}

impl Default for ModelCapability {
    fn default() -> Self {
        Self::TextOnly
    }
}

/// Emergency fallback model when the resolver fails entirely.
///
/// Single source of truth — used by extraction and batch_extraction fallbacks.
/// CT-01: With capability tags, model selection is tag-driven. This constant
/// is only used as a last resort when no models are tagged or installed.
pub const EMERGENCY_FALLBACK_MODEL: &str = "dcarrascosa/medgemma-1.5-4b-it";

// ──────────────────────────────────────────────
// Error Taxonomy (L6-01 dedicated)
// ──────────────────────────────────────────────

/// Dedicated error type for Ollama operations.
///
/// Patient-friendly messages (ACC-L6-01: complete sentences).
/// Separate from `StructuringError` to avoid coupling L6 concerns
/// into the existing L1-03 error hierarchy.
#[derive(Debug, thiserror::Error)]
pub enum OllamaError {
    #[error("Ollama is not running — start Ollama to enable AI features")]
    NotReachable,

    #[error("Ollama returned an error (HTTP {status}): {message}")]
    ApiError { status: u16, message: String },

    #[error("No AI model is installed — pull a model to get started")]
    NoModelAvailable,

    #[error("Model '{0}' is not installed")]
    ModelNotFound(String),

    #[error("Invalid model name: '{0}'")]
    InvalidModelName(String),

    #[error("Only localhost connections are allowed for security")]
    NonLocalEndpoint,

    #[error("Invalid URL format")]
    InvalidUrl,

    #[error("Request timed out after {0} seconds")]
    Timeout(u64),

    #[error("Model download failed: {0}")]
    PullFailed(String),

    #[error("Network error: {0}")]
    Network(String),

    /// R3: Model does not support image/vision inputs.
    #[error("Model '{0}' does not support image inputs — use a vision-capable model like MedGemma")]
    ModelNotVisionCapable(String),

    /// R3: Image exceeds maximum allowed size (20MB base64).
    #[error("Image too large ({0} bytes) — maximum is 20 MB")]
    ImageTooLarge(usize),
}

/// Bridge: convert OllamaError to StructuringError for backward compatibility.
///
/// Existing code uses `StructuringError`. New L6 code uses `OllamaError`.
/// This conversion allows new operations to integrate with existing pipelines.
impl From<OllamaError> for super::StructuringError {
    fn from(err: OllamaError) -> Self {
        match err {
            OllamaError::NotReachable => {
                super::StructuringError::OllamaConnection("localhost:11434".to_string())
            }
            OllamaError::ApiError { status, message } => {
                super::StructuringError::OllamaError { status, body: message }
            }
            OllamaError::NoModelAvailable => super::StructuringError::NoModelAvailable,
            OllamaError::Timeout(secs) => {
                super::StructuringError::HttpClient(format!("Request timed out after {secs}s"))
            }
            OllamaError::Network(msg) => super::StructuringError::HttpClient(msg),
            other => super::StructuringError::HttpClient(other.to_string()),
        }
    }
}

// ──────────────────────────────────────────────
// Security Validators (SEC-L6-01, SEC-L6-02)
// ──────────────────────────────────────────────

/// Validate that a base URL points to localhost only.
///
/// SEC-L6-01: Patient data NEVER leaves the machine via the Ollama client.
/// Accepts: localhost, 127.0.0.1, [::1] (IPv6 loopback).
/// Rejects: any other host, malformed URLs.
pub fn validate_base_url(url: &str) -> Result<(), OllamaError> {
    // Must start with http:// or https://
    let after_scheme = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .ok_or(OllamaError::InvalidUrl)?;

    // Extract host (before port or path)
    let host = after_scheme
        .split(':')
        .next()
        .unwrap_or("")
        .split('/')
        .next()
        .unwrap_or("");

    // Handle IPv6 bracket notation: [::1]
    let host_clean = if after_scheme.starts_with('[') {
        after_scheme
            .split(']')
            .next()
            .unwrap_or("")
            .trim_start_matches('[')
    } else {
        host
    };

    match host_clean {
        "localhost" | "127.0.0.1" | "::1" => Ok(()),
        _ => Err(OllamaError::NonLocalEndpoint),
    }
}

/// Validate a model name against the Ollama naming convention.
///
/// SEC-L6-02: Prevents path traversal, shell injection, and other
/// malicious characters in model names before any HTTP call.
///
/// Supports community namespace format: `namespace/model:tag`
/// Valid: `medgemma:4b`, `MedAIBase/MedGemma1.5:4b`, `alibayram/medgemma`
/// Invalid: `../etc/passwd`, `; rm -rf /`, `a/b/c` (double namespace)
///
/// R-MOD-01: Updated to accept exactly one optional namespace `/` segment.
/// Each segment must start with alphanumeric. Blocks `../`, `./`, `//`,
/// leading/trailing `/`. Model names are used in JSON bodies only (not URL
/// paths), so path traversal risk from `/` is minimal.
pub fn validate_model_name(name: &str) -> Result<(), OllamaError> {
    if name.is_empty() {
        return Err(OllamaError::InvalidModelName(name.to_string()));
    }

    // Format: [namespace/]model[:tag]
    // - namespace: alphanumeric start, then alphanumeric/._-
    // - model: alphanumeric start, then alphanumeric/._-
    // - tag: alphanumeric/._- (after colon)
    // At most ONE `/` allowed (no nested namespaces).
    let valid = regex::Regex::new(
        r"^[a-zA-Z0-9][a-zA-Z0-9._-]*(/[a-zA-Z0-9][a-zA-Z0-9._-]*)?(:[a-zA-Z0-9._-]+)?$",
    )
    .expect("static regex");

    if !valid.is_match(name) {
        return Err(OllamaError::InvalidModelName(name.to_string()));
    }

    Ok(())
}

/// Extract the model name component from a potentially namespaced Ollama model name.
///
/// Strips the namespace prefix (before `/`) and tag suffix (after `:`) to return
/// just the model identity in lowercase. Used by `classify_model()` to match
/// medical prefixes regardless of namespace.
///
/// R-MOD-01 F2: Enables classification of community models like `dcarrascosa/medgemma-1.5-4b-it`.
///
/// # Examples
/// - `"dcarrascosa/medgemma-1.5-4b-it"` → `"medgemma-1.5-4b-it"`
/// - `"medgemma:4b"` → `"medgemma"`
/// - `"llama3.1:8b"` → `"llama3.1"`
/// - `"llama3"` → `"llama3"`
pub fn extract_model_component(full_name: &str) -> String {
    let without_tag = full_name.split(':').next().unwrap_or(full_name);
    let model_part = without_tag.rsplit('/').next().unwrap_or(without_tag);
    model_part.to_lowercase()
}

// ──────────────────────────────────────────────
// Ollama API internal deserialization types
// ──────────────────────────────────────────────

/// Raw response from GET `/api/tags` (Ollama model list).
#[derive(Debug, Deserialize)]
pub(crate) struct OllamaTagsResponse {
    pub models: Vec<OllamaTagModel>,
}

/// Individual model entry in `/api/tags` response.
#[derive(Debug, Deserialize)]
pub(crate) struct OllamaTagModel {
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub digest: String,
    #[serde(default)]
    pub modified_at: String,
    #[serde(default)]
    pub details: Option<OllamaTagDetails>,
}

/// Details sub-object in `/api/tags` response.
#[derive(Debug, Deserialize)]
pub(crate) struct OllamaTagDetails {
    pub family: Option<String>,
    pub parameter_size: Option<String>,
    pub quantization_level: Option<String>,
}

impl From<OllamaTagModel> for ModelInfo {
    fn from(m: OllamaTagModel) -> Self {
        let details = m.details.map(|d| ModelDetails {
            family: d.family,
            parameter_size: d.parameter_size,
            quantization_level: d.quantization_level,
        }).unwrap_or_default();

        ModelInfo {
            name: m.name,
            size: m.size,
            digest: m.digest,
            modified_at: m.modified_at,
            details,
        }
    }
}

/// Raw response from POST `/api/show`.
#[derive(Debug, Deserialize)]
pub(crate) struct OllamaShowResponse {
    pub modelfile: Option<String>,
    pub parameters: Option<String>,
    pub template: Option<String>,
    #[serde(default)]
    pub details: Option<OllamaTagDetails>,
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── CapabilityTag Tests (CT-01) ──

    #[test]
    fn capability_tag_roundtrip() {
        for tag in CapabilityTag::all() {
            let s = tag.to_string();
            let parsed: CapabilityTag = s.parse().unwrap();
            assert_eq!(*tag, parsed);
        }
    }

    #[test]
    fn capability_tag_serde_roundtrip() {
        for tag in CapabilityTag::all() {
            let json = serde_json::to_string(tag).unwrap();
            let parsed: CapabilityTag = serde_json::from_str(&json).unwrap();
            assert_eq!(*tag, parsed);
        }
        // Verify serialization format is uppercase
        assert_eq!(serde_json::to_string(&CapabilityTag::Vision).unwrap(), "\"VISION\"");
        assert_eq!(serde_json::to_string(&CapabilityTag::Txt).unwrap(), "\"TXT\"");
    }

    #[test]
    fn capability_tag_all_has_seven_variants() {
        assert_eq!(CapabilityTag::all().len(), 7);
    }

    #[test]
    fn capability_tag_is_vision_related() {
        assert!(CapabilityTag::Vision.is_vision_related());
        assert!(CapabilityTag::Png.is_vision_related());
        assert!(CapabilityTag::Jpeg.is_vision_related());
        assert!(CapabilityTag::Tiff.is_vision_related());
        assert!(!CapabilityTag::Medical.is_vision_related());
        assert!(!CapabilityTag::Pdf.is_vision_related());
        assert!(!CapabilityTag::Txt.is_vision_related());
    }

    #[test]
    fn capability_tag_from_str_case_insensitive() {
        assert_eq!("vision".parse::<CapabilityTag>().unwrap(), CapabilityTag::Vision);
        assert_eq!("MEDICAL".parse::<CapabilityTag>().unwrap(), CapabilityTag::Medical);
        assert_eq!("Txt".parse::<CapabilityTag>().unwrap(), CapabilityTag::Txt);
        assert!("unknown".parse::<CapabilityTag>().is_err());
    }

    // ── URL Validation Tests (SEC-L6-01) ──

    #[test]
    fn validate_url_accepts_localhost() {
        assert!(validate_base_url("http://localhost:11434").is_ok());
    }

    #[test]
    fn validate_url_accepts_localhost_no_port() {
        assert!(validate_base_url("http://localhost").is_ok());
    }

    #[test]
    fn validate_url_accepts_127_0_0_1() {
        assert!(validate_base_url("http://127.0.0.1:11434").is_ok());
    }

    #[test]
    fn validate_url_accepts_ipv6_loopback() {
        assert!(validate_base_url("http://[::1]:11434").is_ok());
    }

    #[test]
    fn validate_url_rejects_remote_host() {
        assert!(validate_base_url("http://evil.com:11434").is_err());
    }

    #[test]
    fn validate_url_rejects_lan_ip() {
        assert!(validate_base_url("http://192.168.1.100:11434").is_err());
    }

    #[test]
    fn validate_url_rejects_empty() {
        assert!(validate_base_url("").is_err());
    }

    #[test]
    fn validate_url_rejects_no_scheme() {
        assert!(validate_base_url("localhost:11434").is_err());
    }

    #[test]
    fn validate_url_rejects_https_remote() {
        assert!(validate_base_url("https://api.ollama.ai").is_err());
    }

    #[test]
    fn validate_url_accepts_https_localhost() {
        assert!(validate_base_url("https://localhost:11434").is_ok());
    }

    // ── Model Name Validation Tests (SEC-L6-02) ──

    #[test]
    fn validate_name_accepts_simple() {
        assert!(validate_model_name("medgemma").is_ok());
    }

    #[test]
    fn validate_name_accepts_with_tag() {
        assert!(validate_model_name("medgemma:4b").is_ok());
    }

    #[test]
    fn validate_name_accepts_with_dots() {
        assert!(validate_model_name("llama3.1:8b").is_ok());
    }

    #[test]
    fn validate_name_accepts_with_hyphens() {
        assert!(validate_model_name("my-custom-model:latest").is_ok());
    }

    #[test]
    fn validate_name_rejects_empty() {
        assert!(validate_model_name("").is_err());
    }

    #[test]
    fn validate_name_rejects_path_traversal() {
        assert!(validate_model_name("../etc/passwd").is_err());
    }

    #[test]
    fn validate_name_rejects_shell_injection() {
        assert!(validate_model_name("; rm -rf /").is_err());
    }

    #[test]
    fn validate_name_rejects_spaces() {
        assert!(validate_model_name("model name").is_err());
    }

    #[test]
    fn validate_name_rejects_special_chars() {
        assert!(validate_model_name("model@version").is_err());
    }

    #[test]
    fn validate_name_rejects_starts_with_dot() {
        assert!(validate_model_name(".hidden").is_err());
    }

    #[test]
    fn validate_name_rejects_starts_with_hyphen() {
        assert!(validate_model_name("-flag").is_err());
    }

    // ── Namespace Model Validation Tests (R-MOD-01 L.1) ──

    #[test]
    fn validate_name_accepts_namespaced_model() {
        assert!(validate_model_name("MedAIBase/MedGemma1.5:4b").is_ok());
    }

    #[test]
    fn validate_name_accepts_namespaced_no_tag() {
        assert!(validate_model_name("alibayram/medgemma").is_ok());
    }

    #[test]
    fn validate_name_accepts_namespaced_with_dots() {
        assert!(validate_model_name("AntAngelMed/MedGemma1.5:4b").is_ok());
    }

    #[test]
    fn validate_name_rejects_double_slash() {
        assert!(validate_model_name("a//b").is_err());
    }

    #[test]
    fn validate_name_rejects_leading_slash() {
        assert!(validate_model_name("/model").is_err());
    }

    #[test]
    fn validate_name_rejects_trailing_slash() {
        assert!(validate_model_name("model/").is_err());
    }

    #[test]
    fn validate_name_rejects_path_traversal_with_slash() {
        assert!(validate_model_name("../etc/passwd").is_err());
    }

    #[test]
    fn validate_name_rejects_dot_namespace() {
        assert!(validate_model_name("./model").is_err());
    }

    #[test]
    fn validate_name_rejects_double_namespace() {
        // Only one namespace level allowed
        assert!(validate_model_name("a/b/c").is_err());
    }

    // ── extract_model_component Tests (R-MOD-01 L.3) ──

    #[test]
    fn extract_component_from_namespaced_with_tag() {
        assert_eq!(extract_model_component("MedAIBase/MedGemma1.5:4b"), "medgemma1.5");
    }

    #[test]
    fn extract_component_from_namespaced_no_tag() {
        assert_eq!(extract_model_component("alibayram/medgemma"), "medgemma");
    }

    #[test]
    fn extract_component_from_simple_with_tag() {
        assert_eq!(extract_model_component("medgemma:4b"), "medgemma");
    }

    #[test]
    fn extract_component_from_bare_name() {
        assert_eq!(extract_model_component("llama3"), "llama3");
    }

    #[test]
    fn extract_component_preserves_dots() {
        assert_eq!(extract_model_component("llama3.1:8b"), "llama3.1");
    }

    #[test]
    fn extract_component_lowercases() {
        assert_eq!(extract_model_component("BioMistral:7B"), "biomistral");
    }

    #[test]
    fn extract_component_from_new_recommended_models() {
        assert_eq!(extract_model_component("dcarrascosa/medgemma-1.5-4b-it"), "medgemma-1.5-4b-it");
        assert_eq!(extract_model_component("amsaravi/medgemma-4b-it"), "medgemma-4b-it");
    }

    #[test]
    fn extract_component_empty_string() {
        assert_eq!(extract_model_component(""), "");
    }

    // ── Vision Model Detection (R3) ──

    #[test]
    fn is_vision_model_detects_medgemma() {
        assert!(is_vision_model("medgemma:4b"));
        assert!(is_vision_model("dcarrascosa/medgemma-1.5-4b-it"));
        assert!(is_vision_model("amsaravi/medgemma-4b-it"));
        // Legacy names (broken but still detected as vision-capable)
        assert!(is_vision_model("MedAIBase/MedGemma1.5:4b"));
        assert!(is_vision_model("alibayram/medgemma:4b"));
    }

    #[test]
    fn is_vision_model_detects_llava() {
        assert!(is_vision_model("llava:13b"));
    }

    #[test]
    fn is_vision_model_rejects_text_only() {
        assert!(!is_vision_model("llama3:8b"));
        assert!(!is_vision_model("mistral:7b"));
        assert!(!is_vision_model("phi3:mini"));
    }

    // ── Type Deserialization Tests ──

    #[test]
    fn model_info_deserializes_from_ollama_json() {
        let json = r#"{
            "name": "medgemma:4b",
            "size": 2700000000,
            "digest": "sha256:abc123",
            "modified_at": "2025-01-15T10:30:00Z",
            "details": {
                "family": "gemma",
                "parameter_size": "4B",
                "quantization_level": "Q4_K_M"
            }
        }"#;
        let model: OllamaTagModel = serde_json::from_str(json).unwrap();
        let info = ModelInfo::from(model);
        assert_eq!(info.name, "medgemma:4b");
        assert_eq!(info.size, 2_700_000_000);
        assert_eq!(info.details.family.as_deref(), Some("gemma"));
        assert_eq!(info.details.parameter_size.as_deref(), Some("4B"));
    }

    #[test]
    fn model_info_deserializes_without_details() {
        let json = r#"{
            "name": "custom-model:latest",
            "size": 1000000,
            "digest": "sha256:def456",
            "modified_at": "2025-01-10T08:00:00Z"
        }"#;
        let model: OllamaTagModel = serde_json::from_str(json).unwrap();
        let info = ModelInfo::from(model);
        assert_eq!(info.name, "custom-model:latest");
        assert!(info.details.family.is_none());
    }

    #[test]
    fn pull_progress_deserializes_ndjson_download_line() {
        let line = r#"{"status":"downloading digestname","digest":"sha256:abc","total":5000000,"completed":2500000}"#;
        let progress: PullProgress = serde_json::from_str(line).unwrap();
        assert_eq!(progress.status, "downloading digestname");
        assert_eq!(progress.total, Some(5_000_000));
        assert_eq!(progress.completed, Some(2_500_000));
    }

    #[test]
    fn pull_progress_deserializes_ndjson_status_only() {
        let line = r#"{"status":"pulling manifest"}"#;
        let progress: PullProgress = serde_json::from_str(line).unwrap();
        assert_eq!(progress.status, "pulling manifest");
        assert!(progress.total.is_none());
        assert!(progress.completed.is_none());
    }

    #[test]
    fn pull_progress_deserializes_ndjson_success() {
        let line = r#"{"status":"success"}"#;
        let progress: PullProgress = serde_json::from_str(line).unwrap();
        assert_eq!(progress.status, "success");
    }

    #[test]
    fn ollama_health_serializes_correctly() {
        let health = OllamaHealth {
            reachable: true,
            version: Some("0.5.0".to_string()),
            models_count: 3,
        };
        let json = serde_json::to_string(&health).unwrap();
        assert!(json.contains("\"reachable\":true"));
        assert!(json.contains("\"models_count\":3"));
    }

    // ── Error Tests ──

    #[test]
    fn ollama_error_messages_are_sentences() {
        // ACC-L6-01: Error messages must be complete sentences
        let errors = vec![
            OllamaError::NotReachable,
            OllamaError::NoModelAvailable,
            OllamaError::ModelNotFound("test".into()),
            OllamaError::InvalidModelName("bad".into()),
            OllamaError::NonLocalEndpoint,
            OllamaError::Timeout(300),
            OllamaError::PullFailed("disk full".into()),
            OllamaError::Network("connection reset".into()),
        ];
        for err in errors {
            let msg = err.to_string();
            // Every message should start with uppercase and contain a space (sentence)
            assert!(
                msg.len() > 10,
                "Error message too short: {msg}"
            );
        }
    }

    #[test]
    fn ollama_error_converts_to_structuring_error() {
        let err: super::super::StructuringError = OllamaError::NotReachable.into();
        assert!(matches!(err, super::super::StructuringError::OllamaConnection(_)));

        let err: super::super::StructuringError = OllamaError::NoModelAvailable.into();
        assert!(matches!(err, super::super::StructuringError::NoModelAvailable));
    }

    // ── Generation Options ──

    #[test]
    fn generation_options_default_medical_grade() {
        let opts = GenerationOptions::default();
        assert!((opts.temperature - 0.1).abs() < f32::EPSILON, "Medical default temperature should be 0.1");
        assert!((opts.top_p - 0.9).abs() < f32::EPSILON);
        assert_eq!(opts.top_k, 40);
        assert!(opts.num_predict.is_none());
    }

    #[test]
    fn generation_options_serializes_correctly() {
        let opts = GenerationOptions {
            temperature: 0.5,
            top_p: 0.9,
            top_k: 50,
            num_predict: Some(2048),
            num_ctx: Some(4096),
        };
        let json = serde_json::to_value(&opts).unwrap();
        // f32 precision: compare as f64 with tolerance
        let temp = json["temperature"].as_f64().unwrap();
        assert!((temp - 0.5).abs() < 0.001, "temperature should be ~0.5");
        let top_p = json["top_p"].as_f64().unwrap();
        assert!((top_p - 0.9).abs() < 0.001, "top_p should be ~0.9");
        assert_eq!(json["top_k"], 50);
        assert_eq!(json["num_predict"], 2048);
    }

    #[test]
    fn generation_options_num_predict_none_serializes() {
        let opts = GenerationOptions::default();
        let json = serde_json::to_value(&opts).unwrap();
        // num_predict should be present as null when using serde default
        assert!(json.get("num_predict").is_some());
    }

}
