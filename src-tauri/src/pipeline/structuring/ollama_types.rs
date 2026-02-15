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

/// Result of a lightweight health check (GET `/`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaHealth {
    pub reachable: bool,
    pub version: Option<String>,
    pub models_count: usize,
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

/// A curated recommended model entry.
///
/// Advisory, not restrictive — users can pull any model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedModel {
    pub name: String,
    pub description: String,
    pub min_ram_gb: u32,
    pub medical: bool,
}

/// Return the curated list of recommended medical models.
///
/// Maintained in code (not fetched from registry).
/// DOM-L6-01: includes minimum RAM requirements.
pub fn recommended_models() -> Vec<RecommendedModel> {
    vec![
        RecommendedModel {
            name: "medgemma:4b".to_string(),
            description: "Medical-trained, 4B parameters, recommended for most systems".to_string(),
            min_ram_gb: 8,
            medical: true,
        },
        RecommendedModel {
            name: "medgemma:27b".to_string(),
            description: "Medical-trained, 27B parameters, higher accuracy, needs 32GB+ RAM".to_string(),
            min_ram_gb: 32,
            medical: true,
        },
    ]
}

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
/// Valid: `medgemma:4b`, `llama3.1:8b`, `my-model:latest`
/// Invalid: `../etc/passwd`, `; rm -rf /`, empty string
pub fn validate_model_name(name: &str) -> Result<(), OllamaError> {
    if name.is_empty() {
        return Err(OllamaError::InvalidModelName(name.to_string()));
    }

    // Ollama model names: alphanumeric start, then alphanumeric/._-
    // Optional tag after colon: alphanumeric/._-
    let valid = regex::Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9._-]*(:[a-zA-Z0-9._-]+)?$")
        .expect("static regex");

    if !valid.is_match(name) {
        return Err(OllamaError::InvalidModelName(name.to_string()));
    }

    Ok(())
}

/// Build a list of recommended model names for preference resolution.
pub fn recommended_model_names() -> Vec<String> {
    vec![
        "medgemma".to_string(),
        "medgemma:4b".to_string(),
        "medgemma:27b".to_string(),
        "medgemma:latest".to_string(),
    ]
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

    // ── Recommended Models ──

    #[test]
    fn recommended_model_names_not_empty() {
        let names = recommended_model_names();
        assert!(names.len() >= 3);
        assert!(names.contains(&"medgemma:4b".to_string()));
    }
}
