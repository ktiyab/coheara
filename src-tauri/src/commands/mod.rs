pub mod ai_setup;
pub mod appointment;
pub mod chat;
pub mod coherence;
pub mod devices;
pub mod distribution;
pub mod extraction;
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

/// S.1: Granular AI status level for frontend display.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusLevel {
    /// Status has not been checked yet
    Unknown,
    /// Ollama is reachable but no model configured
    Reachable,
    /// Model is configured but generation not verified
    Configured,
    /// Model can generate text (full verification passed)
    Verified,
    /// Previously verified, but a recent operation failed
    Degraded,
    /// Ollama not reachable or other fatal error
    Error,
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
    /// S.1: Granular status level for frontend routing.
    pub level: StatusLevel,
}

/// Proactive check of AI service availability (IMP-015).
///
/// Called by the frontend on app load / Home screen to show the user
/// whether AI chat is functional before they attempt a conversation.
/// Uses L6-01 health_check() for reachability and L6-04 resolver for model.
/// Runs on a blocking thread to avoid freezing the UI (HTTP calls to Ollama).
#[tauri::command]
pub async fn check_ai_status(
    state: State<'_, Arc<CoreState>>,
) -> Result<AiStatus, String> {
    let state = state.inner().clone();
    Ok(tauri::async_runtime::spawn_blocking(move || {
        let client = crate::ollama_service::OllamaService::client();

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

        // S.1: Compute granular status level (with cached verification)
        let level = match (ollama_available, &active_model) {
            (false, _) => {
                state.set_ai_verified(false);
                StatusLevel::Error
            }
            (true, None) => StatusLevel::Reachable,
            (true, Some(_)) if state.is_ai_verified() => StatusLevel::Verified,
            (true, Some(_)) => StatusLevel::Configured,
        };

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
            level,
        }
    })
    .await
    .unwrap_or_else(|_| AiStatus {
        ollama_available: false,
        active_model: None,
        embedder_type: "mock".to_string(),
        summary: "Status check failed".to_string(),
        level: StatusLevel::Error,
    }))
}

/// S.1+S.7: Verify AI generation capability and update cached status.
///
/// Runs a lightweight test generation against the resolved model.
/// On success, promotes status from `Configured` to `Verified`.
/// On failure, clears the verified flag so status shows `Configured` or `Degraded`.
///
/// Frontend should call this once after startup (with ~30s delay) and
/// periodically (every 60s) to maintain accurate status.
/// Runs on a blocking thread to avoid freezing the UI (LLM generate call).
#[tauri::command]
pub async fn verify_ai_status(
    state: State<'_, Arc<CoreState>>,
) -> Result<AiStatus, String> {
    let state = state.inner().clone();
    Ok(tauri::async_runtime::spawn_blocking(move || {
        use crate::pipeline::structuring::types::LlmClient;

        let client = crate::ollama_service::OllamaService::client();

        // Quick health check first
        let health = client.health_check().ok();
        let ollama_available = health.as_ref().is_some_and(|h| h.reachable);

        if !ollama_available {
            state.set_ai_verified(false);
            return AiStatus {
                ollama_available: false,
                active_model: None,
                embedder_type: detect_embedder_type(),
                summary: "Ollama not detected — install Ollama and pull a model".to_string(),
                level: StatusLevel::Error,
            };
        }

        // Try to resolve active model
        let active_model = state
            .open_db()
            .ok()
            .and_then(|conn| state.resolver().resolve(&conn, &client).ok());

        let model_name = active_model.as_ref().map(|m| m.name.clone());

        // Try to acquire Ollama exclusively for verification — skip if busy
        let guard = model_name.as_ref().and_then(|name| {
            state.ollama().try_acquire(
                crate::ollama_service::OperationKind::ModelVerification,
                name,
            )
        });

        // Attempt verification only if we got the lock (Ollama not busy)
        let verified = match (&guard, &model_name) {
            (Some(_), Some(name)) => {
                match client.generate(
                    name,
                    "Reply with exactly: OK",
                    "You are a test. Reply only with OK.",
                ) {
                    Ok(response) => response.trim().contains("OK"),
                    Err(e) => {
                        tracing::warn!(model = %name, error = %e, "S.1: AI verification failed");
                        false
                    }
                }
            }
            (None, Some(_)) => {
                // Ollama is busy with a real operation — the running operation IS the proof
                tracing::info!(
                    busy_with = ?state.ollama().current_operation(),
                    "Ollama busy, skipping verification — active use implies working"
                );
                // Return current verified state without changing it
                return build_status(&state, ollama_available, active_model);
            }
            _ => false,
        };

        state.set_ai_verified(verified);
        build_status(&state, ollama_available, active_model)
    })
    .await
    .unwrap_or_else(|_| AiStatus {
        ollama_available: false,
        active_model: None,
        embedder_type: "mock".to_string(),
        summary: "Status check failed".to_string(),
        level: StatusLevel::Error,
    }))
}

/// Build AiStatus from current state.
fn build_status(
    state: &Arc<CoreState>,
    ollama_available: bool,
    active_model: Option<crate::pipeline::structuring::preferences::ResolvedModel>,
) -> AiStatus {
    let verified = state.is_ai_verified();
    let embedder_type = detect_embedder_type();

    let level = match (ollama_available, &active_model) {
        (false, _) => StatusLevel::Error,
        (true, None) => StatusLevel::Reachable,
        (true, Some(_)) if verified => StatusLevel::Verified,
        (true, Some(_)) => StatusLevel::Configured,
    };

    let summary = match (&level, &active_model, embedder_type.as_str()) {
        (StatusLevel::Verified, Some(model), "onnx") => format!("AI verified — {} + ONNX embeddings", model.name),
        (StatusLevel::Verified, Some(model), _) => format!("AI verified — {} (semantic search limited)", model.name),
        (StatusLevel::Configured, Some(model), _) => format!("AI configured — {} (generation not verified)", model.name),
        (StatusLevel::Reachable, _, _) => "Ollama running — no model selected. Set up AI in Settings.".to_string(),
        _ => "Ollama not detected — install Ollama and pull a model".to_string(),
    };

    AiStatus {
        ollama_available,
        active_model,
        embedder_type,
        summary,
        level,
    }
}

/// Query the current Ollama operation (if any).
///
/// Returns `None` if Ollama is idle, or the active operation's kind, model, and start time.
/// Used by the frontend AI status indicator to show "Processing document..." etc.
#[tauri::command]
pub fn ollama_current_operation(
    state: State<'_, Arc<CoreState>>,
) -> Option<crate::ollama_service::ActiveOperation> {
    state.ollama().current_operation()
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
            level: StatusLevel::Error,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"ollama_available\":false"));
        assert!(json.contains("\"active_model\":null"));
        assert!(json.contains("\"embedder_type\":\"mock\""));
        assert!(json.contains("\"level\":\"error\""));
    }

    #[test]
    fn status_level_serializes_snake_case() {
        let json = serde_json::to_string(&StatusLevel::Verified).unwrap();
        assert_eq!(json, "\"verified\"");
        let json = serde_json::to_string(&StatusLevel::Degraded).unwrap();
        assert_eq!(json, "\"degraded\"");
        let json = serde_json::to_string(&StatusLevel::Configured).unwrap();
        assert_eq!(json, "\"configured\"");
    }

    #[test]
    fn detect_embedder_type_returns_mock_without_onnx() {
        let t = detect_embedder_type();
        // Default build without ONNX model files on disk
        assert_eq!(t, "mock");
    }

    #[test]
    fn verified_status_with_model() {
        let status = AiStatus {
            ollama_available: true,
            active_model: Some(ResolvedModel {
                name: "medgemma:latest".into(),
                quality: crate::pipeline::structuring::preferences::ModelQuality::Medical,
                source: crate::pipeline::structuring::preferences::PreferenceSource::User,
            }),
            embedder_type: "mock".to_string(),
            summary: "AI verified — medgemma:latest".to_string(),
            level: StatusLevel::Verified,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"level\":\"verified\""));
        assert!(json.contains("\"ollama_available\":true"));
    }
}
