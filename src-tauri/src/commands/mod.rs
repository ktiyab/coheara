pub mod ai_setup;
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

use std::sync::Arc;

use tauri::State;

use crate::core_state::CoreState;
use crate::pipeline::structuring::preferences::ResolvedModel;

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
    /// Resolved active model with quality and source (L6-04 §11).
    pub active_model: Option<ResolvedModel>,
    /// Embedding backend: "onnx" or "mock".
    pub embedder_type: String,
    /// Human-readable status summary.
    pub summary: String,
}

/// Proactive check of AI service availability (IMP-015).
///
/// Called by the frontend on app load / Home screen to show the user
/// whether AI chat is functional before they attempt a conversation.
/// Uses L6-01 health_check() for reachability and L6-04 resolver for model.
#[tauri::command]
pub fn check_ai_status(
    state: State<'_, Arc<CoreState>>,
) -> AiStatus {
    use crate::pipeline::structuring::ollama::OllamaClient;

    let client = OllamaClient::default_local();

    // Check Ollama reachability via health check (L6-01)
    let health = client.health_check().ok();
    let ollama_available = health.as_ref().is_some_and(|h| h.reachable);

    // Try to resolve active model via preferences (L6-04)
    let active_model = if ollama_available {
        state
            .open_db()
            .ok()
            .and_then(|conn| {
                state.resolver().resolve(&conn, &client).ok()
            })
    } else {
        None
    };

    // Check embedder
    let embedder_type = detect_embedder_type();

    let summary = match (ollama_available, &active_model, embedder_type.as_str()) {
        (true, Some(model), "onnx") => format!("AI ready — {} + ONNX embeddings", model.name),
        (true, Some(model), _) => format!("AI ready — {} (semantic search limited)", model.name),
        (true, None, _) => "Ollama running — no model selected. Set up AI in Settings.".to_string(),
        (false, _, _) => "Ollama not detected — install Ollama and pull a model".to_string(),
    };

    AiStatus {
        ollama_available,
        active_model,
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
    fn ai_status_struct_serializes() {
        let status = AiStatus {
            ollama_available: false,
            active_model: None,
            embedder_type: "mock".to_string(),
            summary: "Ollama not detected".to_string(),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"ollama_available\":false"));
        assert!(json.contains("\"active_model\":null"));
        assert!(json.contains("\"embedder_type\":\"mock\""));
    }

    #[test]
    fn detect_embedder_type_returns_mock_without_onnx() {
        let t = detect_embedder_type();
        // Default build without ONNX model files on disk
        assert_eq!(t, "mock");
    }
}
