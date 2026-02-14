pub mod appointment;
pub mod chat;
pub mod coherence;
pub mod devices;
pub mod distribution;
pub mod home;
pub mod import;
pub mod journal;
pub mod medications;
pub mod mobile_api;
pub mod pairing;
pub mod profile;
pub mod review;
pub mod state;
pub mod sync;
pub mod timeline;
pub mod transfer;
pub mod trust;

/// Health check IPC command — verifies backend is running
#[tauri::command]
pub fn health_check() -> String {
    tracing::debug!("Health check called");
    "ok".to_string()
}

/// AI service availability for the frontend status indicator.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AiStatus {
    /// Whether Ollama is reachable on localhost:11434.
    pub ollama_available: bool,
    /// Which MedGemma model was detected (if any).
    pub ollama_model: Option<String>,
    /// Embedding backend: "onnx" or "mock".
    pub embedder_type: String,
    /// Human-readable status summary.
    pub summary: String,
}

/// Proactive check of AI service availability (IMP-015).
///
/// Called by the frontend on app load / Home screen to show the user
/// whether AI chat is functional before they attempt a conversation.
#[tauri::command]
pub fn check_ai_status() -> AiStatus {
    use crate::pipeline::structuring::ollama::OllamaClient;

    // Check Ollama
    let client = OllamaClient::default_local();
    let (ollama_available, ollama_model) = match client.find_best_model() {
        Ok(model) => (true, Some(model)),
        Err(_) => (false, None),
    };

    // Check embedder
    let embedder_type = detect_embedder_type();

    let summary = match (ollama_available, embedder_type.as_str()) {
        (true, "onnx") => format!(
            "AI ready — {} + ONNX embeddings",
            ollama_model.as_deref().unwrap_or("unknown")
        ),
        (true, _) => format!(
            "AI ready — {} (semantic search limited)",
            ollama_model.as_deref().unwrap_or("unknown")
        ),
        (false, _) => "Ollama not detected — install Ollama and pull a MedGemma model".to_string(),
    };

    AiStatus {
        ollama_available,
        ollama_model,
        embedder_type,
        summary,
    }
}

/// Detect which embedding backend will be used at runtime.
fn detect_embedder_type() -> String {
    #[cfg(feature = "onnx-embeddings")]
    {
        let model_dir = crate::config::embedding_model_dir();
        if model_dir.join("model.onnx").exists() && model_dir.join("tokenizer.json").exists() {
            return "onnx".to_string();
        }
    }
    "mock".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_check_returns_ok() {
        assert_eq!(health_check(), "ok");
    }

    #[test]
    fn ai_status_returns_valid_structure() {
        let status = check_ai_status();
        // In CI/test, Ollama is not running
        assert!(!status.summary.is_empty());
        assert!(status.embedder_type == "mock" || status.embedder_type == "onnx");
    }

    #[test]
    fn detect_embedder_type_returns_mock_without_onnx() {
        let t = detect_embedder_type();
        // Default build without ONNX model files on disk
        assert_eq!(t, "mock");
    }
}
