use serde::{Deserialize, Serialize};
use std::io::BufRead;

use super::StructuringError;
use super::ollama_types::{
    GenerationOptions, ModelDetail, ModelInfo, ModelPreference, OllamaError, OllamaHealth,
    OllamaShowResponse, OllamaTagsResponse, PullProgress,
    recommended_model_names, validate_base_url, validate_model_name,
};
use super::types::LlmClient;

/// Default generation timeout in seconds.
/// MedGemma cold start can take ~5 minutes (observed 4m59s in R-MOD-04).
/// 600s provides sufficient headroom for cold start + generation.
const DEFAULT_GENERATE_TIMEOUT_SECS: u64 = 600;

/// Ollama HTTP client for local LLM inference.
///
/// Owns two HTTP clients with different timeouts:
/// - `client`: 600s for generation requests (MedGemma can be slow on cold start)
/// - `client_quick`: 5s for health checks, model listing, show operations
///
/// SD-03: Blocking client stays blocking. Tauri commands run on a threadpool.
/// Async is only used for streaming pull (via tokio::spawn in IPC layer).
pub struct OllamaClient {
    base_url: String,
    /// Long-timeout client for generation (600s default).
    client: reqwest::blocking::Client,
    /// Short-timeout client for health/list/show (5s).
    client_quick: reqwest::blocking::Client,
    /// Moderate-timeout client for delete (30s).
    client_moderate: reqwest::blocking::Client,
    timeout_secs: u64,
    /// Generation parameters (temperature, top_p, top_k, etc.).
    options: GenerationOptions,
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
            options: GenerationOptions::default(),
        }
    }

    /// Set generation options (temperature, top_p, top_k, num_predict).
    pub fn with_options(mut self, options: GenerationOptions) -> Self {
        self.options = options;
        self
    }

    /// Get a reference to the current generation options.
    pub fn generation_options(&self) -> &GenerationOptions {
        &self.options
    }

    /// Default Ollama instance at localhost:11434 with 10-minute generation timeout.
    pub fn default_local() -> Self {
        Self::new("http://localhost:11434", DEFAULT_GENERATE_TIMEOUT_SECS)
    }

    /// Create an OllamaClient from the `OLLAMA_HOST` environment variable.
    ///
    /// SEC-L6-01: Validates localhost-only policy before accepting.
    /// Falls back to `default_local()` if env var is not set or invalid.
    pub fn from_env() -> Self {
        match std::env::var("OLLAMA_HOST") {
            Ok(host) => {
                // Normalize: if no scheme, add http://
                let url = if host.starts_with("http://") || host.starts_with("https://") {
                    host
                } else {
                    format!("http://{host}")
                };

                match validate_base_url(&url) {
                    Ok(()) => {
                        tracing::info!(ollama_host = %url, "Using OLLAMA_HOST from environment");
                        Self::new(&url, DEFAULT_GENERATE_TIMEOUT_SECS)
                    }
                    Err(_) => {
                        tracing::warn!(
                            ollama_host = %url,
                            "OLLAMA_HOST rejected by localhost-only policy, using default"
                        );
                        Self::default_local()
                    }
                }
            }
            Err(_) => Self::default_local(),
        }
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
        let _span = tracing::info_span!("ollama_health_check", base_url = %self.base_url).entered();
        let start = std::time::Instant::now();
        tracing::info!("Ollama health check starting");

        // GET / returns "Ollama is running" with 200 OK
        let url = format!("{}/", self.base_url);
        let response = self.client_quick.get(&url).send().map_err(|e| {
            let err = if e.is_connect() || e.is_timeout() {
                OllamaError::NotReachable
            } else {
                OllamaError::Network(e.to_string())
            };
            tracing::warn!(elapsed_ms = %start.elapsed().as_millis(), error = %e, "Ollama health check failed");
            err
        })?;

        if !response.status().is_success() {
            tracing::warn!(
                status = response.status().as_u16(),
                elapsed_ms = %start.elapsed().as_millis(),
                "Ollama health check: non-success status"
            );
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

        tracing::info!(
            elapsed_ms = %start.elapsed().as_millis(),
            models_count,
            "Ollama health check complete"
        );

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
        let _span = tracing::info_span!("ollama_list_models").entered();
        let start = std::time::Instant::now();
        let url = format!("{}/api/tags", self.base_url);

        let response = self.client_quick.get(&url).send().map_err(|e| {
            let err = if e.is_connect() || e.is_timeout() {
                OllamaError::NotReachable
            } else {
                OllamaError::Network(e.to_string())
            };
            tracing::warn!(elapsed_ms = %start.elapsed().as_millis(), error = %e, "Ollama list_models failed");
            err
        })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            tracing::warn!(status = status.as_u16(), "Ollama list_models: non-success status");
            return Err(OllamaError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        let parsed: OllamaTagsResponse = response
            .json()
            .map_err(|e| OllamaError::Network(format!("Failed to parse model list: {e}")))?;

        let models: Vec<ModelInfo> = parsed.models.into_iter().map(ModelInfo::from).collect();
        tracing::info!(
            elapsed_ms = %start.elapsed().as_millis(),
            count = models.len(),
            "Ollama list_models complete"
        );
        Ok(models)
    }

    /// Get detailed information about a specific model.
    ///
    /// Uses POST `/api/show` with 10s timeout.
    pub fn show_model(&self, name: &str) -> Result<ModelDetail, OllamaError> {
        validate_model_name(name)?;
        let _span = tracing::info_span!("ollama_show_model", model = %name).entered();
        let start = std::time::Instant::now();
        tracing::info!("Ollama show_model starting");

        let url = format!("{}/api/show", self.base_url);

        let response = self
            .client_quick
            .post(&url)
            .json(&serde_json::json!({ "name": name }))
            .send()
            .map_err(|e| {
                let err = if e.is_connect() || e.is_timeout() {
                    OllamaError::NotReachable
                } else {
                    OllamaError::Network(e.to_string())
                };
                tracing::warn!(model = %name, elapsed_ms = %start.elapsed().as_millis(), error = %e, "Ollama show_model failed");
                err
            })?;

        let status = response.status();
        if !status.is_success() {
            tracing::warn!(model = %name, status = status.as_u16(), "Ollama show_model: non-success status");
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

        tracing::info!(model = %name, elapsed_ms = %start.elapsed().as_millis(), "Ollama show_model complete");

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
        let _span = tracing::info_span!("ollama_pull_model", model = %name).entered();
        let start = std::time::Instant::now();
        tracing::info!("Ollama pull_model starting");

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
                        tracing::info!(model = %name, elapsed_ms = %start.elapsed().as_millis(), "Ollama pull_model complete");
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
        let _span = tracing::info_span!("ollama_delete_model", model = %name).entered();
        let start = std::time::Instant::now();
        tracing::info!("Ollama delete_model starting");

        let url = format!("{}/api/delete", self.base_url);

        let response = self
            .client_moderate
            .delete(&url)
            .json(&serde_json::json!({ "name": name }))
            .send()
            .map_err(|e| {
                let err = if e.is_connect() || e.is_timeout() {
                    OllamaError::NotReachable
                } else {
                    OllamaError::Network(e.to_string())
                };
                tracing::warn!(model = %name, elapsed_ms = %start.elapsed().as_millis(), error = %e, "Ollama delete_model failed");
                err
            })?;

        let status = response.status();
        if !status.is_success() {
            tracing::warn!(model = %name, status = status.as_u16(), "Ollama delete_model: non-success status");
            if status.as_u16() == 404 {
                return Err(OllamaError::ModelNotFound(name.to_string()));
            }
            let body = response.text().unwrap_or_default();
            return Err(OllamaError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        tracing::info!(model = %name, elapsed_ms = %start.elapsed().as_millis(), "Ollama delete_model complete");
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
        let _span = tracing::info_span!("ollama_resolve_model").entered();
        let start = std::time::Instant::now();
        let available = self.list_models_detailed()?;

        if available.is_empty() {
            tracing::warn!(elapsed_ms = %start.elapsed().as_millis(), "Ollama resolve_model: no models available");
            return Err(OllamaError::NoModelAvailable);
        }

        let available_names: Vec<&str> = available.iter().map(|m| m.name.as_str()).collect();

        // Step 1: User-selected model
        if let Some(ref selected) = preference.user_selected {
            if available_names.iter().any(|n| n.starts_with(selected.as_str())) {
                tracing::info!(model = %selected, source = "user", elapsed_ms = %start.elapsed().as_millis(), "Ollama resolve_model complete");
                return Ok(selected.clone());
            }
            tracing::debug!(model = %selected, "User-selected model not available, trying fallbacks");
        }

        // Step 2: Recommended medical models
        for rec in &preference.recommended {
            if available_names.iter().any(|n| n.starts_with(rec.as_str())) {
                tracing::info!(model = %rec, source = "recommended", elapsed_ms = %start.elapsed().as_millis(), "Ollama resolve_model complete");
                return Ok(rec.clone());
            }
        }

        // Also check default recommended list if preference.recommended is empty
        if preference.recommended.is_empty() {
            for rec in recommended_model_names() {
                if available_names.iter().any(|n| n.starts_with(rec.as_str())) {
                    tracing::info!(model = %rec, source = "default_recommended", elapsed_ms = %start.elapsed().as_millis(), "Ollama resolve_model complete");
                    return Ok(rec);
                }
            }
        }

        // Step 3: Fallback to any available model
        if preference.fallback_any {
            if let Some(first) = available.first() {
                tracing::info!(model = %first.name, source = "fallback_any", elapsed_ms = %start.elapsed().as_millis(), "Ollama resolve_model complete");
                return Ok(first.name.clone());
            }
        }

        tracing::warn!(elapsed_ms = %start.elapsed().as_millis(), "Ollama resolve_model: no suitable model found");
        Err(OllamaError::NoModelAvailable)
    }

    /// Generate a response with streaming — tokens arrive as they're produced.
    ///
    /// Sends NDJSON chunks via `token_tx`. Each chunk contains a partial response token.
    /// The caller (IPC layer) forwards these as Tauri events to the frontend.
    ///
    /// Uses the long-timeout client. No retry logic — streaming is for interactive use
    /// where the user can see progress and retry manually if needed.
    pub fn generate_streaming(
        &self,
        model: &str,
        prompt: &str,
        system: &str,
        token_tx: std::sync::mpsc::Sender<String>,
    ) -> Result<String, StructuringError> {
        validate_model_name(model).map_err(|e| StructuringError::HttpClient(e.to_string()))?;
        let _span = tracing::info_span!("ollama_generate_streaming", model = %model, prompt_len = prompt.len()).entered();
        let start = std::time::Instant::now();
        tracing::info!("Ollama generate_streaming starting");

        let url = format!("{}/api/generate", self.base_url);
        let body = serde_json::json!({
            "model": model,
            "prompt": prompt,
            "system": system,
            "stream": true,
            "keep_alive": "0",
            "options": {
                "temperature": self.options.temperature,
                "top_p": self.options.top_p,
                "top_k": self.options.top_k,
            }
        });

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
                        "Streaming request timed out after {}s",
                        self.timeout_secs
                    ))
                } else {
                    StructuringError::HttpClient(e.to_string())
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().unwrap_or_default();
            return Err(StructuringError::OllamaError {
                status: status.as_u16(),
                body: body_text,
            });
        }

        // Stream NDJSON — each line contains a partial response token
        let reader = std::io::BufReader::new(response);
        let mut full_response = String::new();

        for line_result in reader.lines() {
            let line = line_result.map_err(|e| StructuringError::HttpClient(e.to_string()))?;

            if line.trim().is_empty() {
                continue;
            }

            #[derive(Deserialize)]
            struct StreamChunk {
                response: String,
                #[serde(default)]
                done: bool,
            }

            match serde_json::from_str::<StreamChunk>(&line) {
                Ok(chunk) => {
                    full_response.push_str(&chunk.response);

                    // Send token to receiver; if send fails, receiver dropped (cancelled)
                    if token_tx.send(chunk.response).is_err() {
                        tracing::info!("Streaming generation cancelled by receiver");
                        return Ok(full_response);
                    }

                    if chunk.done {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!(line = %line, error = %e, "Unparseable NDJSON chunk during streaming");
                }
            }
        }

        tracing::info!(
            model = %model,
            elapsed_ms = %start.elapsed().as_millis(),
            response_len = full_response.len(),
            "Ollama generate_streaming complete"
        );

        Ok(full_response)
    }
}

/// Request body for Ollama /api/generate
#[derive(Serialize)]
struct OllamaGenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    system: &'a str,
    stream: bool,
    options: OllamaGenerateOptions,
    /// SEC-02-G09: Unload model after each inference to clear context memory.
    /// Ollama retains context in GPU/RAM between requests; setting keep_alive to "0"
    /// forces immediate unload, preventing PHI from lingering in model memory.
    /// Accepted trade-off: slightly slower cold-start on next request (~1-3s model reload).
    keep_alive: &'a str,
}

/// Ollama generation options (nested in request body).
#[derive(Serialize)]
struct OllamaGenerateOptions {
    temperature: f32,
    top_p: f32,
    top_k: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
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

/// Maximum number of retry attempts for transient generate failures.
const MAX_GENERATE_RETRIES: u32 = 2;
/// Initial retry delay in seconds (doubles on each retry).
const INITIAL_RETRY_DELAY_SECS: u64 = 10;

impl LlmClient for OllamaClient {
    fn generate(
        &self,
        model: &str,
        prompt: &str,
        system: &str,
    ) -> Result<String, StructuringError> {
        let _span = tracing::info_span!("ollama_generate", model = %model, prompt_len = prompt.len()).entered();
        let start = std::time::Instant::now();
        tracing::info!("Ollama generate starting");

        let url = format!("{}/api/generate", self.base_url);

        let mut last_error = None;

        for attempt in 0..=MAX_GENERATE_RETRIES {
            if attempt > 0 {
                let delay = INITIAL_RETRY_DELAY_SECS * 2u64.pow(attempt - 1);
                tracing::warn!(
                    model = %model,
                    attempt = attempt + 1,
                    delay_secs = delay,
                    "Ollama generate: retrying after transient failure"
                );
                std::thread::sleep(std::time::Duration::from_secs(delay));
            }

            let body = OllamaGenerateRequest {
                model,
                prompt,
                system,
                stream: false,
                options: OllamaGenerateOptions {
                    temperature: self.options.temperature,
                    top_p: self.options.top_p,
                    top_k: self.options.top_k,
                    num_predict: self.options.num_predict,
                },
                keep_alive: "0",
            };

            let response = match self.client.post(&url).json(&body).send() {
                Ok(resp) => resp,
                Err(e) => {
                    if e.is_connect() {
                        // Connection refused — Ollama is not running, don't retry
                        tracing::warn!(model = %model, error = %e, "Ollama generate: connection refused, not retrying");
                        return Err(StructuringError::OllamaConnection(self.base_url.clone()));
                    }
                    if e.is_timeout() {
                        // Timeout is retryable
                        last_error = Some(StructuringError::HttpClient(format!(
                            "Request timed out after {}s",
                            self.timeout_secs
                        )));
                        continue;
                    }
                    // Other network errors — retry
                    last_error = Some(StructuringError::HttpClient(e.to_string()));
                    continue;
                }
            };

            let status = response.status();
            if status.is_success() {
                let parsed: OllamaGenerateResponse = response
                    .json()
                    .map_err(|e| StructuringError::ResponseParsing(e.to_string()))?;

                tracing::info!(
                    model = %model,
                    elapsed_ms = %start.elapsed().as_millis(),
                    response_len = parsed.response.len(),
                    attempts = attempt + 1,
                    "Ollama generate complete"
                );

                return Ok(parsed.response);
            }

            let status_code = status.as_u16();
            let body_text = response.text().unwrap_or_default();

            // Retryable server errors: 500, 503
            if status_code == 500 || status_code == 503 {
                tracing::warn!(
                    model = %model,
                    status = status_code,
                    attempt = attempt + 1,
                    "Ollama generate: server error, will retry"
                );
                last_error = Some(StructuringError::OllamaError {
                    status: status_code,
                    body: body_text,
                });
                continue;
            }

            // Non-retryable errors (400, 404, etc.)
            tracing::warn!(
                model = %model,
                status = status_code,
                elapsed_ms = %start.elapsed().as_millis(),
                "Ollama generate: non-retryable error"
            );
            return Err(StructuringError::OllamaError {
                status: status_code,
                body: body_text,
            });
        }

        // All retries exhausted
        tracing::warn!(
            model = %model,
            elapsed_ms = %start.elapsed().as_millis(),
            max_retries = MAX_GENERATE_RETRIES,
            "Ollama generate: all retries exhausted"
        );
        Err(last_error.unwrap_or_else(|| {
            StructuringError::HttpClient("All retries exhausted".to_string())
        }))
    }

    fn is_model_available(&self, model: &str) -> Result<bool, StructuringError> {
        let models = self.list_models()?;
        let available = models.iter().any(|m| m.starts_with(model));
        tracing::debug!(model = %model, available, "Ollama is_model_available check");
        Ok(available)
    }

    fn list_models(&self) -> Result<Vec<String>, StructuringError> {
        let start = std::time::Instant::now();
        let url = format!("{}/api/tags", self.base_url);

        let response = self.client_quick.get(&url).send().map_err(|e| {
            let err = if e.is_connect() {
                StructuringError::OllamaConnection(self.base_url.clone())
            } else {
                StructuringError::HttpClient(e.to_string())
            };
            tracing::warn!(elapsed_ms = %start.elapsed().as_millis(), error = %e, "Ollama list_models (legacy) failed");
            err
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

        let names: Vec<String> = parsed.models.into_iter().map(|m| m.name).collect();
        tracing::debug!(count = names.len(), elapsed_ms = %start.elapsed().as_millis(), "Ollama list_models (legacy) complete");
        Ok(names)
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
    fn default_local_uses_600s_timeout() {
        let client = OllamaClient::default_local();
        assert_eq!(client.timeout_secs, 600);
    }

    #[test]
    fn default_generation_options_medical_grade() {
        let client = OllamaClient::default_local();
        let opts = client.generation_options();
        assert!((opts.temperature - 0.1).abs() < f32::EPSILON);
        assert!((opts.top_p - 0.9).abs() < f32::EPSILON);
        assert_eq!(opts.top_k, 40);
        assert!(opts.num_predict.is_none());
    }

    #[test]
    fn with_options_overrides_defaults() {
        let custom = GenerationOptions {
            temperature: 0.7,
            top_p: 0.95,
            top_k: 50,
            num_predict: Some(1024),
        };
        let client = OllamaClient::default_local().with_options(custom);
        let opts = client.generation_options();
        assert!((opts.temperature - 0.7).abs() < f32::EPSILON);
        assert_eq!(opts.num_predict, Some(1024));
    }

    #[test]
    fn base_url_accessor() {
        let client = OllamaClient::default_local();
        assert_eq!(client.base_url(), "http://localhost:11434");
    }

    // ── streaming tests ──

    #[test]
    fn generate_streaming_rejects_invalid_model_name() {
        let client = OllamaClient::new("http://localhost:99999", 1);
        let (tx, _rx) = std::sync::mpsc::channel();
        let result = client.generate_streaming("../evil", "prompt", "system", tx);
        assert!(result.is_err());
    }

    #[test]
    fn generate_streaming_fails_when_unreachable() {
        let client = OllamaClient::new("http://localhost:99999", 1);
        let (tx, _rx) = std::sync::mpsc::channel();
        let result = client.generate_streaming("test-model", "prompt", "system", tx);
        assert!(result.is_err());
    }

    // ── retry constants ──

    #[test]
    fn retry_constants_reasonable() {
        assert_eq!(MAX_GENERATE_RETRIES, 2, "Should retry at most 2 times");
        assert_eq!(INITIAL_RETRY_DELAY_SECS, 10, "Initial delay should be 10s");
        // Total worst-case delay: 10s + 20s = 30s
        let total_delay: u64 = (0..MAX_GENERATE_RETRIES)
            .map(|i| INITIAL_RETRY_DELAY_SECS * 2u64.pow(i))
            .sum();
        assert_eq!(total_delay, 30, "Total retry delay should be 30s");
    }

    // ── from_env / URL validation tests ──
    // Note: env var tests that set OLLAMA_HOST are unsafe in parallel.
    // We test the URL validation logic directly instead.

    #[test]
    fn validate_base_url_accepts_localhost() {
        assert!(validate_base_url("http://localhost:11434").is_ok());
        assert!(validate_base_url("http://localhost:9999").is_ok());
        assert!(validate_base_url("http://127.0.0.1:11434").is_ok());
    }

    #[test]
    fn validate_base_url_rejects_remote() {
        assert!(validate_base_url("http://192.168.1.50:11434").is_err());
        assert!(validate_base_url("http://example.com:11434").is_err());
    }

    #[test]
    fn from_env_uses_default_when_unset() {
        // OLLAMA_HOST is not set in CI/test environments
        // from_env should fall back to default
        let client = OllamaClient::from_env();
        // Either uses env var (if set to localhost) or defaults
        assert!(
            client.base_url.contains("localhost") || client.base_url.contains("127.0.0.1"),
            "from_env should use localhost: {}",
            client.base_url
        );
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
