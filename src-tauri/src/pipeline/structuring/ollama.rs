use serde::{Deserialize, Serialize};
use std::io::BufRead;

use super::StructuringError;
use super::ollama_types::{
    ModelDetail, ModelInfo, ModelPreference, OllamaError, OllamaHealth,
    OllamaShowResponse, OllamaTagsResponse, PullProgress,
    recommended_model_names, validate_model_name,
};
use super::types::LlmClient;

/// Ollama HTTP client for local LLM inference.
///
/// Owns two HTTP clients with different timeouts:
/// - `client`: 300s for generation requests (MedGemma can be slow on cold start)
/// - `client_quick`: 5s for health checks, model listing, show operations
///
/// SD-03: Blocking client stays blocking. Tauri commands run on a threadpool.
/// Async is only used for streaming pull (via tokio::spawn in IPC layer).
pub struct OllamaClient {
    base_url: String,
    /// Long-timeout client for generation (300s default).
    client: reqwest::blocking::Client,
    /// Short-timeout client for health/list/show (5s).
    client_quick: reqwest::blocking::Client,
    /// Moderate-timeout client for delete (30s).
    client_moderate: reqwest::blocking::Client,
    timeout_secs: u64,
}

impl OllamaClient {
    /// Create a new OllamaClient pointing at a local Ollama instance.
    ///
    /// SD-02: Separate timeout tiers.
    /// - `timeout_secs`: used for generation requests
    /// - Health/list/show: hardcoded 5s (quick fail if unreachable)
    /// - Delete: hardcoded 30s (filesystem operation)
    pub fn new(base_url: &str, timeout_secs: u64) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        let client_quick = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("Failed to create quick HTTP client");

        let client_moderate = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create moderate HTTP client");

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
            client_quick,
            client_moderate,
            timeout_secs,
        }
    }

    /// Default Ollama instance at localhost:11434 with 5-minute generation timeout.
    pub fn default_local() -> Self {
        Self::new("http://localhost:11434", 300)
    }

    /// The base URL of this client.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    // ──────────────────────────────────────────────
    // L6-01: New Operations (OllamaOperations)
    // ──────────────────────────────────────────────

    /// Lightweight health check — verifies Ollama is reachable.
    ///
    /// PERF-L6-01: 5s timeout. Returns within 5s even if unreachable.
    /// Follows up with model count for the health summary.
    pub fn health_check(&self) -> Result<OllamaHealth, OllamaError> {
        // GET / returns "Ollama is running" with 200 OK
        let url = format!("{}/", self.base_url);
        let response = self.client_quick.get(&url).send().map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                OllamaError::NotReachable
            } else {
                OllamaError::Network(e.to_string())
            }
        })?;

        if !response.status().is_success() {
            return Ok(OllamaHealth {
                reachable: false,
                version: None,
                models_count: 0,
            });
        }

        // Try to get version from the response body ("Ollama is running")
        let body = response.text().unwrap_or_default();
        let version = if body.contains("Ollama") {
            // Ollama doesn't return version in the root endpoint body,
            // but the response confirms it's running.
            None
        } else {
            None
        };

        // Get model count via list
        let models_count = match self.list_models_detailed() {
            Ok(models) => models.len(),
            Err(_) => 0,
        };

        Ok(OllamaHealth {
            reachable: true,
            version,
            models_count,
        })
    }

    /// List all locally installed models with enriched metadata.
    ///
    /// SD-04: Returns `Vec<ModelInfo>` with name, size, family, quantization.
    /// Uses 5s timeout (quick client).
    pub fn list_models_detailed(&self) -> Result<Vec<ModelInfo>, OllamaError> {
        let url = format!("{}/api/tags", self.base_url);

        let response = self.client_quick.get(&url).send().map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                OllamaError::NotReachable
            } else {
                OllamaError::Network(e.to_string())
            }
        })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(OllamaError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        let parsed: OllamaTagsResponse = response
            .json()
            .map_err(|e| OllamaError::Network(format!("Failed to parse model list: {e}")))?;

        Ok(parsed.models.into_iter().map(ModelInfo::from).collect())
    }

    /// Get detailed information about a specific model.
    ///
    /// Uses POST `/api/show` with 10s timeout.
    pub fn show_model(&self, name: &str) -> Result<ModelDetail, OllamaError> {
        validate_model_name(name)?;

        let url = format!("{}/api/show", self.base_url);

        let response = self
            .client_quick
            .post(&url)
            .json(&serde_json::json!({ "name": name }))
            .send()
            .map_err(|e| {
                if e.is_connect() || e.is_timeout() {
                    OllamaError::NotReachable
                } else {
                    OllamaError::Network(e.to_string())
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            if status.as_u16() == 404 {
                return Err(OllamaError::ModelNotFound(name.to_string()));
            }
            let body = response.text().unwrap_or_default();
            return Err(OllamaError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        let parsed: OllamaShowResponse = response
            .json()
            .map_err(|e| OllamaError::Network(format!("Failed to parse model detail: {e}")))?;

        let details = parsed.details.map(|d| super::ollama_types::ModelDetails {
            family: d.family,
            parameter_size: d.parameter_size,
            quantization_level: d.quantization_level,
        }).unwrap_or_default();

        Ok(ModelDetail {
            name: name.to_string(),
            modelfile: parsed.modelfile,
            parameters: parsed.parameters,
            template: parsed.template,
            details,
        })
    }

    /// Initiate model download. Sends `PullProgress` events via the channel.
    ///
    /// Streams NDJSON from POST `/api/pull`. Each line is a `PullProgress`.
    /// The caller (IPC layer) forwards these as Tauri events to the frontend.
    ///
    /// No timeout — downloads are arbitrarily long.
    /// Cancellation: drop the `progress_tx` sender to signal cancellation.
    pub fn pull_model(
        &self,
        name: &str,
        progress_tx: std::sync::mpsc::Sender<PullProgress>,
    ) -> Result<(), OllamaError> {
        validate_model_name(name)?;

        let url = format!("{}/api/pull", self.base_url);

        // Use a client without timeout for pull (downloads can be very long)
        let no_timeout_client = reqwest::blocking::Client::builder()
            .build()
            .map_err(|e| OllamaError::Network(e.to_string()))?;

        let response = no_timeout_client
            .post(&url)
            .json(&serde_json::json!({ "name": name }))
            .send()
            .map_err(|e| {
                if e.is_connect() {
                    OllamaError::NotReachable
                } else {
                    OllamaError::PullFailed(e.to_string())
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(OllamaError::PullFailed(format!(
                "HTTP {}: {body}",
                status.as_u16()
            )));
        }

        // Stream NDJSON — each line is a PullProgress JSON object
        let reader = std::io::BufReader::new(response);
        for line_result in reader.lines() {
            let line = line_result.map_err(|e| OllamaError::PullFailed(e.to_string()))?;

            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<PullProgress>(&line) {
                Ok(progress) => {
                    let is_success = progress.status == "success";
                    // If send fails, the receiver was dropped (cancellation)
                    if progress_tx.send(progress).is_err() {
                        tracing::info!("Model pull cancelled by receiver");
                        return Ok(());
                    }
                    if is_success {
                        return Ok(());
                    }
                }
                Err(e) => {
                    tracing::warn!(line = %line, error = %e, "Unparseable NDJSON line during pull");
                    // Continue — some lines may be status messages we don't parse
                }
            }
        }

        Ok(())
    }

    /// Delete a locally installed model.
    ///
    /// Uses DELETE `/api/delete` with 30s timeout.
    pub fn delete_model(&self, name: &str) -> Result<(), OllamaError> {
        validate_model_name(name)?;

        let url = format!("{}/api/delete", self.base_url);

        let response = self
            .client_moderate
            .delete(&url)
            .json(&serde_json::json!({ "name": name }))
            .send()
            .map_err(|e| {
                if e.is_connect() || e.is_timeout() {
                    OllamaError::NotReachable
                } else {
                    OllamaError::Network(e.to_string())
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            if status.as_u16() == 404 {
                return Err(OllamaError::ModelNotFound(name.to_string()));
            }
            let body = response.text().unwrap_or_default();
            return Err(OllamaError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        Ok(())
    }

    /// Resolve the best available model given user preferences.
    ///
    /// SD-05: Resolution chain:
    /// 1. User-selected model (if available)
    /// 2. Recommended medical models (first match)
    /// 3. Any available model (if fallback_any is true)
    /// 4. Error: NoModelAvailable
    pub fn resolve_model(&self, preference: &ModelPreference) -> Result<String, OllamaError> {
        let available = self.list_models_detailed()?;

        if available.is_empty() {
            return Err(OllamaError::NoModelAvailable);
        }

        let available_names: Vec<&str> = available.iter().map(|m| m.name.as_str()).collect();

        // Step 1: User-selected model
        if let Some(ref selected) = preference.user_selected {
            if available_names.iter().any(|n| n.starts_with(selected.as_str())) {
                return Ok(selected.clone());
            }
            tracing::debug!(model = %selected, "User-selected model not available, trying fallbacks");
        }

        // Step 2: Recommended medical models
        for rec in &preference.recommended {
            if available_names.iter().any(|n| n.starts_with(rec.as_str())) {
                return Ok(rec.clone());
            }
        }

        // Also check default recommended list if preference.recommended is empty
        if preference.recommended.is_empty() {
            for rec in recommended_model_names() {
                if available_names.iter().any(|n| n.starts_with(rec.as_str())) {
                    return Ok(rec);
                }
            }
        }

        // Step 3: Fallback to any available model
        if preference.fallback_any {
            if let Some(first) = available.first() {
                return Ok(first.name.clone());
            }
        }

        Err(OllamaError::NoModelAvailable)
    }
}

/// Request body for Ollama /api/generate
#[derive(Serialize)]
struct OllamaGenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    system: &'a str,
    stream: bool,
}

/// Response body from Ollama /api/generate
#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

/// Legacy response body from Ollama /api/tags (simple name-only).
/// Kept for backward compatibility with existing LlmClient::list_models().
#[derive(Deserialize)]
struct LegacyOllamaTagsResponse {
    models: Vec<LegacyOllamaModel>,
}

#[derive(Deserialize)]
struct LegacyOllamaModel {
    name: String,
}

impl LlmClient for OllamaClient {
    fn generate(
        &self,
        model: &str,
        prompt: &str,
        system: &str,
    ) -> Result<String, StructuringError> {
        let url = format!("{}/api/generate", self.base_url);
        let body = OllamaGenerateRequest {
            model,
            prompt,
            system,
            stream: false,
        };

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| {
                if e.is_connect() {
                    StructuringError::OllamaConnection(self.base_url.clone())
                } else if e.is_timeout() {
                    StructuringError::HttpClient(format!(
                        "Request timed out after {}s",
                        self.timeout_secs
                    ))
                } else {
                    StructuringError::HttpClient(e.to_string())
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(StructuringError::OllamaError {
                status: status.as_u16(),
                body,
            });
        }

        let parsed: OllamaGenerateResponse = response
            .json()
            .map_err(|e| StructuringError::ResponseParsing(e.to_string()))?;

        Ok(parsed.response)
    }

    fn is_model_available(&self, model: &str) -> Result<bool, StructuringError> {
        let models = self.list_models()?;
        Ok(models.iter().any(|m| m.starts_with(model)))
    }

    fn list_models(&self) -> Result<Vec<String>, StructuringError> {
        let url = format!("{}/api/tags", self.base_url);

        let response = self.client_quick.get(&url).send().map_err(|e| {
            if e.is_connect() {
                StructuringError::OllamaConnection(self.base_url.clone())
            } else {
                StructuringError::HttpClient(e.to_string())
            }
        })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(StructuringError::OllamaError {
                status: status.as_u16(),
                body,
            });
        }

        let parsed: LegacyOllamaTagsResponse = response
            .json()
            .map_err(|e| StructuringError::ResponseParsing(e.to_string()))?;

        Ok(parsed.models.into_iter().map(|m| m.name).collect())
    }
}

/// Mock LLM client for testing — returns a configurable response.
pub struct MockLlmClient {
    response: String,
    available_models: Vec<String>,
}

impl MockLlmClient {
    pub fn new(response: &str) -> Self {
        Self {
            response: response.to_string(),
            available_models: vec!["medgemma:latest".to_string()],
        }
    }

    pub fn with_models(mut self, models: Vec<String>) -> Self {
        self.available_models = models;
        self
    }
}

impl LlmClient for MockLlmClient {
    fn generate(
        &self,
        _model: &str,
        _prompt: &str,
        _system: &str,
    ) -> Result<String, StructuringError> {
        Ok(self.response.clone())
    }

    fn is_model_available(&self, model: &str) -> Result<bool, StructuringError> {
        Ok(self.available_models.iter().any(|m| m.starts_with(model)))
    }

    fn list_models(&self) -> Result<Vec<String>, StructuringError> {
        Ok(self.available_models.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::ollama_types::ModelPreference;

    #[test]
    fn mock_client_returns_configured_response() {
        let client = MockLlmClient::new("test response");
        let result = client.generate("model", "prompt", "system").unwrap();
        assert_eq!(result, "test response");
    }

    #[test]
    fn mock_client_lists_models() {
        let client = MockLlmClient::new("").with_models(vec![
            "medgemma:latest".into(),
            "llama3:8b".into(),
        ]);
        let models = client.list_models().unwrap();
        assert_eq!(models.len(), 2);
        assert!(client.is_model_available("medgemma").unwrap());
    }

    #[test]
    fn mock_client_model_not_available() {
        let client = MockLlmClient::new("").with_models(vec!["llama3:8b".into()]);
        assert!(!client.is_model_available("medgemma").unwrap());
    }

    #[test]
    fn ollama_client_constructor() {
        let client = OllamaClient::new("http://localhost:11434", 120);
        assert_eq!(client.base_url, "http://localhost:11434");
        assert_eq!(client.timeout_secs, 120);
    }

    #[test]
    fn ollama_client_trims_trailing_slash() {
        let client = OllamaClient::new("http://localhost:11434/", 60);
        assert_eq!(client.base_url, "http://localhost:11434");
    }

    #[test]
    fn default_local_uses_standard_port() {
        let client = OllamaClient::default_local();
        assert_eq!(client.base_url, "http://localhost:11434");
    }

    #[test]
    fn base_url_accessor() {
        let client = OllamaClient::default_local();
        assert_eq!(client.base_url(), "http://localhost:11434");
    }

    // ── resolve_model tests (B10) ──
    // Note: These test the resolution LOGIC, not HTTP calls.
    // We test by checking error paths (no Ollama running in test env).

    #[test]
    fn resolve_model_returns_not_reachable_without_ollama() {
        let client = OllamaClient::new("http://localhost:99999", 1);
        let pref = ModelPreference {
            user_selected: Some("medgemma:4b".into()),
            recommended: vec![],
            fallback_any: true,
        };
        // Should fail because Ollama is not running on port 99999
        let result = client.resolve_model(&pref);
        assert!(result.is_err());
    }

    #[test]
    fn model_preference_default_has_fallback() {
        let pref = ModelPreference::default();
        assert!(pref.user_selected.is_none());
        assert!(pref.recommended.is_empty());
        assert!(!pref.fallback_any); // Default: don't accept any model
    }
}
