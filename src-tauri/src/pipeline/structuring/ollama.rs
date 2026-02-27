use std::error::Error as StdError;

use serde::{Deserialize, Serialize};
use std::io::BufRead;

use super::StructuringError;
use super::ollama_types::{
    GenerationOptions, ModelCapability, ModelDetail, ModelInfo, ModelPreference,
    OllamaError, OllamaHealth, OllamaShowResponse, OllamaTagsResponse, PullProgress,
    VisionChatMessage, VisionChatRequest, VisionGenerateRequest, VisionGenerationOptions,
    VISION_MODEL_PREFIXES,
    extract_model_component, validate_base_url, validate_model_name,
};
use super::types::{LlmClient, VisionClient};

/// Ollama HTTP client for local LLM inference.
///
/// Completion-based operations — no artificial request timeouts.
/// Operations run until Ollama responds (success or error).
///
/// - `connect_timeout(10s)`: Fast detection of "Ollama not running"
/// - `tcp_keepalive(15s)`: Prevents WSL2/network bridge from dropping idle connections
///   during long CPU inference (MedGemma can take minutes per page on CPU)
/// - Health/list/show: 5s per-request timeout (management operations)
/// - Delete: 30s per-request timeout (filesystem operation)
/// - Pull: no timeout (downloads are arbitrarily long)
/// - Generation/vision: no timeout (inference takes as long as needed)
///
/// SD-03: Blocking client stays blocking. Tauri commands run on a threadpool.
/// Async is only used for streaming pull (via tokio::spawn in IPC layer).
pub struct OllamaClient {
    base_url: String,
    /// Single shared HTTP client — connect_timeout only, no global request timeout.
    client: reqwest::blocking::Client,
    /// Generation parameters (temperature, top_p, top_k, etc.).
    options: GenerationOptions,
    /// Context window for vision calls (hardware-tiered). None = model default.
    vision_num_ctx: Option<u32>,
}

impl OllamaClient {
    /// Create a new OllamaClient pointing at a local Ollama instance.
    ///
    /// SD-02: Single client with connect_timeout for fast failure detection.
    /// No request timeouts — inference operations complete or fail naturally.
    /// - Health/list/show: 5s hardcoded at call sites
    /// - Delete: 30s hardcoded at call sites
    /// - Pull: no timeout (applied at call site)
    pub fn new(base_url: &str) -> Self {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .tcp_keepalive(std::time::Duration::from_secs(15))
            .no_proxy()
            .build()
            .expect("Failed to create HTTP client");

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
            options: GenerationOptions::default(),
            vision_num_ctx: None,
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

    /// Set the context window size for vision calls (hardware-tiered).
    pub fn set_vision_num_ctx(&mut self, num_ctx: u32) {
        self.vision_num_ctx = Some(num_ctx);
    }

    /// Default Ollama instance at localhost:11434.
    pub fn default_local() -> Self {
        Self::new("http://localhost:11434")
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
                        Self::new(&url)
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
    // Model pre-warming
    // ──────────────────────────────────────────────

    /// Pre-load a model into memory without generating any output.
    ///
    /// Sends an empty generate request that triggers model loading but produces
    /// no tokens. The model stays loaded for `keep_alive` duration (30 minutes).
    ///
    /// Call this before the first inference to avoid the cold-start timeout
    /// that occurs when model loading exceeds the connection idle threshold.
    /// Subsequent requests to a loaded model respond immediately.
    pub fn warm_model(&self, model: &str) -> Result<(), OllamaError> {
        validate_model_name(model)?;
        let _span = tracing::info_span!("ollama_warm_model", model = %model).entered();
        let start = std::time::Instant::now();
        tracing::info!("Pre-warming model (loading into memory)");

        let url = format!("{}/api/generate", self.base_url);
        let body = serde_json::json!({
            "model": model,
            "keep_alive": "30m"
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| {
                tracing::warn!(
                    model = %model,
                    elapsed_ms = %start.elapsed().as_millis(),
                    error = %e,
                    cause = ?e.source(),
                    "Model pre-warm failed"
                );
                if e.is_connect() || e.is_timeout() {
                    OllamaError::NotReachable
                } else {
                    OllamaError::Network(e.to_string())
                }
            })?;

        if !response.status().is_success() {
            let body = response.text().unwrap_or_default();
            tracing::warn!(
                model = %model,
                elapsed_ms = %start.elapsed().as_millis(),
                body = %body,
                "Model pre-warm: non-success status"
            );
            return Err(OllamaError::ApiError {
                status: 500,
                message: body,
            });
        }

        // Consume the response body (streaming or non-streaming)
        let _ = response.text();

        tracing::info!(
            model = %model,
            elapsed_ms = %start.elapsed().as_millis(),
            "Model pre-warmed successfully"
        );

        Ok(())
    }

    /// Unload a model from memory by setting keep_alive to "0".
    ///
    /// Used by the CPU swap strategy to free RAM between extraction and structuring.
    pub fn unload_model(&self, model: &str) -> Result<(), OllamaError> {
        validate_model_name(model)?;
        let _span = tracing::info_span!("ollama_unload_model", model = %model).entered();
        let start = std::time::Instant::now();
        tracing::info!("Unloading model from memory");

        let url = format!("{}/api/generate", self.base_url);
        let body = serde_json::json!({
            "model": model,
            "keep_alive": "0"
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| {
                tracing::warn!(model = %model, error = %e, "Model unload failed");
                if e.is_connect() || e.is_timeout() {
                    OllamaError::NotReachable
                } else {
                    OllamaError::Network(e.to_string())
                }
            })?;

        // Consume the response body
        let _ = response.text();

        tracing::info!(
            model = %model,
            elapsed_ms = %start.elapsed().as_millis(),
            "Model unloaded from memory"
        );

        Ok(())
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
        let response = self.client.get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .map_err(|e| {
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

    /// List currently loaded (running) models with GPU/CPU allocation.
    ///
    /// Calls GET `/api/ps` with 5s timeout. Returns which models are
    /// loaded in memory and how much VRAM each uses. This is the basis
    /// for hardware detection (GPU vs CPU inference).
    pub fn list_running_models(&self) -> Result<Vec<super::ollama_types::RunningModelInfo>, OllamaError> {
        let _span = tracing::info_span!("ollama_list_running").entered();
        let start = std::time::Instant::now();
        let url = format!("{}/api/ps", self.base_url);

        let response = self.client.get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .map_err(|e| {
                let err = if e.is_connect() || e.is_timeout() {
                    OllamaError::NotReachable
                } else {
                    OllamaError::Network(e.to_string())
                };
                tracing::warn!(elapsed_ms = %start.elapsed().as_millis(), error = %e, "Ollama list_running failed");
                err
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(OllamaError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        #[derive(Deserialize)]
        struct PsResponse {
            #[serde(default)]
            models: Vec<PsModel>,
        }

        #[derive(Deserialize)]
        struct PsModel {
            name: String,
            #[serde(default)]
            size: u64,
            #[serde(default)]
            size_vram: u64,
            #[serde(default)]
            processor: String,
        }

        let parsed: PsResponse = response
            .json()
            .map_err(|e| OllamaError::Network(format!("Failed to parse /api/ps response: {e}")))?;

        let models = parsed.models.into_iter().map(|m| {
            super::ollama_types::RunningModelInfo {
                name: m.name,
                size: m.size,
                size_vram: m.size_vram,
                processor: m.processor,
            }
        }).collect();

        tracing::info!(
            elapsed_ms = %start.elapsed().as_millis(),
            "Ollama list_running complete"
        );

        Ok(models)
    }

    /// List all locally installed models with enriched metadata.
    ///
    /// SD-04: Returns `Vec<ModelInfo>` with name, size, family, quantization.
    /// Uses 5s timeout (quick client).
    pub fn list_models_detailed(&self) -> Result<Vec<ModelInfo>, OllamaError> {
        let _span = tracing::info_span!("ollama_list_models").entered();
        let start = std::time::Instant::now();
        let url = format!("{}/api/tags", self.base_url);

        let response = self.client.get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .map_err(|e| {
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
            .client
            .post(&url)
            .timeout(std::time::Duration::from_secs(5))
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

        // No per-request timeout — downloads can be arbitrarily long.
        let response = self.client
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
            .client
            .delete(&url)
            .timeout(std::time::Duration::from_secs(30))
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
        let mut options = serde_json::json!({
            "temperature": self.options.temperature,
            "top_p": self.options.top_p,
            "top_k": self.options.top_k,
        });
        if let Some(num_ctx) = self.options.num_ctx {
            options["num_ctx"] = serde_json::json!(num_ctx);
        }
        let body = serde_json::json!({
            "model": model,
            "prompt": prompt,
            "system": system,
            "stream": true,
            "keep_alive": "30m",
            "options": options,
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| {
                if e.is_connect() || e.is_timeout() {
                    StructuringError::OllamaConnection(self.base_url.clone())
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

    /// L6-06: Streaming generation with StreamGuard degeneration watchdog.
    ///
    /// Wraps `generate_streaming` with a `StreamGuard` that monitors the token
    /// stream for repetition patterns and aborts early on degeneration.
    ///
    /// Returns the full (healthy) response, or `StructuringError::Degeneration`
    /// if the guard detects a degeneration pattern.
    pub fn generate_streaming_guarded(
        &self,
        model: &str,
        prompt: &str,
        system: &str,
        token_tx: std::sync::mpsc::Sender<String>,
        guard_config: crate::pipeline::stream_guard::StreamGuardConfig,
    ) -> Result<String, StructuringError> {
        use crate::pipeline::stream_guard::StreamGuard;

        let mut guard = StreamGuard::new(guard_config);

        // Create an intermediate channel to intercept tokens
        let (inner_tx, inner_rx) = std::sync::mpsc::channel::<String>();

        // Run the streaming generation in the current thread
        // but with the inner_tx sender. We process tokens as they arrive.
        validate_model_name(model).map_err(|e| StructuringError::HttpClient(e.to_string()))?;
        let _span = tracing::info_span!("ollama_generate_streaming_guarded", model = %model).entered();
        let start = std::time::Instant::now();

        let url = format!("{}/api/generate", self.base_url);
        let mut options = serde_json::json!({
            "temperature": self.options.temperature,
            "top_p": self.options.top_p,
            "top_k": self.options.top_k,
        });
        if let Some(num_ctx) = self.options.num_ctx {
            options["num_ctx"] = serde_json::json!(num_ctx);
        }
        let body = serde_json::json!({
            "model": model,
            "prompt": prompt,
            "system": system,
            "stream": true,
            "keep_alive": "30m",
            "options": options,
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| {
                if e.is_connect() || e.is_timeout() {
                    StructuringError::OllamaConnection(self.base_url.clone())
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

        // Stream NDJSON with StreamGuard monitoring
        let reader = std::io::BufReader::new(response);
        // inner_tx/inner_rx not used — we process inline
        drop(inner_tx);
        drop(inner_rx);

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
                    // Feed token to StreamGuard before forwarding
                    if let Err(abort) = guard.feed(&chunk.response) {
                        tracing::warn!(
                            pattern = %abort.pattern,
                            tokens = abort.tokens_before_abort,
                            "StreamGuard detected degeneration — aborting stream"
                        );
                        return Err(StructuringError::Degeneration {
                            pattern: abort.pattern.to_string(),
                            tokens_before_abort: abort.tokens_before_abort,
                            partial_output: abort.partial_output,
                        });
                    }

                    // Token is healthy — forward to consumer
                    if token_tx.send(chunk.response).is_err() {
                        tracing::info!("Streaming generation cancelled by receiver");
                        return Ok(guard.accumulated_output().to_string());
                    }

                    if chunk.done {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!(line = %line, error = %e, "Unparseable NDJSON chunk during guarded streaming");
                }
            }
        }

        tracing::info!(
            model = %model,
            elapsed_ms = %start.elapsed().as_millis(),
            tokens = guard.total_tokens(),
            "Ollama generate_streaming_guarded complete"
        );

        Ok(guard.accumulated_output().to_string())
    }
}

// ──────────────────────────────────────────────
// R3: Vision model support
// ──────────────────────────────────────────────

/// Maximum base64-encoded image size: 20 MB.
///
/// Ollama has no documented limit, but 20 MB base64 ≈ 15 MB raw image,
/// which covers even high-DPI page renders (200 DPI A4 ≈ 1-3 MB PNG).
const MAX_IMAGE_SIZE_BYTES: usize = 20 * 1024 * 1024;

// No per-request timeout for vision operations — inference runs to completion.
// connect_timeout(10s) on the HTTP client handles "Ollama not running" detection.

impl OllamaClient {
    /// Detect whether a model supports vision (image) inputs.
    ///
    /// R3: Two-step detection:
    /// 1. Fast heuristic — check model name against known vision prefixes.
    /// 2. Slow fallback — query `/api/show` for projector/multimodal indicators.
    ///
    /// Conservative: returns `TextOnly` if detection fails (e.g., Ollama unreachable).
    pub fn detect_capability(&self, model_name: &str) -> Result<ModelCapability, OllamaError> {
        let component = extract_model_component(model_name);

        // Fast path: known vision model prefixes
        for prefix in VISION_MODEL_PREFIXES {
            if component.starts_with(prefix) {
                tracing::debug!(model = %model_name, "Vision capability detected via name prefix");
                return Ok(ModelCapability::Vision);
            }
        }

        // Slow path: query /api/show for multimodal indicators
        match self.show_model(model_name) {
            Ok(detail) => {
                if let Some(ref modelfile) = detail.modelfile {
                    let lower = modelfile.to_lowercase();
                    // Projector layers indicate vision adapter (LLaVA-style)
                    if lower.contains("projector")
                        || lower.contains("mmproj")
                        || lower.contains("vision")
                    {
                        tracing::debug!(model = %model_name, "Vision capability detected via modelfile");
                        return Ok(ModelCapability::Vision);
                    }
                }
                Ok(ModelCapability::TextOnly)
            }
            Err(e) => {
                tracing::warn!(model = %model_name, error = %e, "Could not query model detail, assuming TextOnly");
                Ok(ModelCapability::TextOnly)
            }
        }
    }
}

// ──────────────────────────────────────────────
// Streaming NDJSON collectors
//
// Reusable functions that read NDJSON lines from a streaming Ollama response,
// accumulate tokens, and return the full text. Streaming prevents the OS idle
// socket timeout that kills non-streaming requests on Windows/WSL2 after 30s.
// ──────────────────────────────────────────────

/// Collect a streaming `/api/generate` response into a single string.
///
/// Each NDJSON line: `{ "response": "token", "done": false }`
/// Final line: `{ "response": "", "done": true }`
fn collect_generate_stream(
    response: reqwest::blocking::Response,
    model: &str,
    start: &std::time::Instant,
) -> Result<String, OllamaError> {
    #[derive(Deserialize)]
    struct GenerateChunk {
        response: String,
        #[serde(default)]
        done: bool,
    }

    let reader = std::io::BufReader::new(response);
    let mut full_response = String::new();

    for line_result in reader.lines() {
        let line = line_result.map_err(|e| {
            tracing::warn!(
                model = %model,
                elapsed_ms = %start.elapsed().as_millis(),
                error = %e,
                "Stream read error during generate_with_images"
            );
            OllamaError::Network(format!("Stream read error: {e}"))
        })?;

        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<GenerateChunk>(&line) {
            Ok(chunk) => {
                full_response.push_str(&chunk.response);
                if chunk.done {
                    break;
                }
            }
            Err(e) => {
                tracing::warn!(
                    line = %line,
                    error = %e,
                    "Unparseable NDJSON line during vision generate"
                );
            }
        }
    }

    Ok(full_response)
}

/// Collect a streaming `/api/chat` response into a single string.
///
/// Each NDJSON line: `{ "message": { "content": "token" }, "done": false }`
/// Final line: `{ "message": { "content": "" }, "done": true }`
fn collect_chat_stream(
    response: reqwest::blocking::Response,
    model: &str,
    start: &std::time::Instant,
) -> Result<String, OllamaError> {
    #[derive(Deserialize)]
    struct ChatChunk {
        message: Option<ChatChunkMessage>,
        #[serde(default)]
        done: bool,
    }

    #[derive(Deserialize)]
    struct ChatChunkMessage {
        content: String,
    }

    let reader = std::io::BufReader::new(response);
    let mut full_response = String::new();

    for line_result in reader.lines() {
        let line = line_result.map_err(|e| {
            tracing::warn!(
                model = %model,
                elapsed_ms = %start.elapsed().as_millis(),
                error = %e,
                "Stream read error during chat_with_images"
            );
            OllamaError::Network(format!("Stream read error: {e}"))
        })?;

        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<ChatChunk>(&line) {
            Ok(chunk) => {
                if let Some(msg) = chunk.message {
                    full_response.push_str(&msg.content);
                }
                if chunk.done {
                    break;
                }
            }
            Err(e) => {
                tracing::warn!(
                    line = %line,
                    error = %e,
                    "Unparseable NDJSON line during vision chat"
                );
            }
        }
    }

    Ok(full_response)
}

impl VisionClient for OllamaClient {
    /// Generate text from a prompt with base64-encoded images via Ollama `/api/generate`.
    ///
    /// R4: Used for document OCR (MedGemma default) and medical image interpretation.
    ///
    /// Key behaviors:
    /// - Image size guard: rejects images > 20 MB base64
    /// - Deterministic: temperature=0.0, num_predict=8192
    /// - keep_alive=30m: model stays loaded (caller must warm_model() first)
    /// - Streaming: uses `stream: true` to prevent OS idle socket timeout
    ///   (with `stream: false`, zero bytes flow during inference and the OS
    ///   kills the connection at its default 30s timeout on Windows/WSL2)
    fn generate_with_images(
        &self,
        model: &str,
        prompt: &str,
        images: &[String],
        system: Option<&str>,
    ) -> Result<String, OllamaError> {
        validate_model_name(model)?;
        let _span = tracing::info_span!(
            "ollama_generate_with_images",
            model = %model,
            prompt_len = prompt.len(),
            image_count = images.len(),
        )
        .entered();
        let start = std::time::Instant::now();
        tracing::info!("Ollama generate_with_images starting (streaming)");

        // Guard: reject oversized images
        for (i, img) in images.iter().enumerate() {
            if img.len() > MAX_IMAGE_SIZE_BYTES {
                tracing::warn!(
                    image_index = i,
                    size = img.len(),
                    max = MAX_IMAGE_SIZE_BYTES,
                    "Image exceeds maximum size"
                );
                return Err(OllamaError::ImageTooLarge(img.len()));
            }
        }

        let request = VisionGenerateRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            system: system.map(|s| s.to_string()),
            images: images.to_vec(),
            stream: true,
            options: Some(VisionGenerationOptions {
                temperature: 0.0,
                num_predict: 8192,
                num_ctx: self.vision_num_ctx,
            }),
            keep_alive: Some("30m".to_string()),
        };

        let url = format!("{}/api/generate", self.base_url);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .map_err(|e| {
                let err = if e.is_connect() || e.is_timeout() {
                    OllamaError::NotReachable
                } else {
                    OllamaError::Network(e.to_string())
                };
                tracing::warn!(
                    model = %model,
                    elapsed_ms = %start.elapsed().as_millis(),
                    error = %e,
                    cause = ?e.source(),
                    "Ollama generate_with_images failed"
                );
                err
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            tracing::warn!(
                model = %model,
                status = status.as_u16(),
                elapsed_ms = %start.elapsed().as_millis(),
                body = %body,
                "Ollama generate_with_images: non-success status"
            );
            return Err(OllamaError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        // Stream NDJSON — each line: { "response": "token", "done": false }
        // Data flows continuously, preventing OS idle socket timeout.
        let full_response = collect_generate_stream(response, model, &start)?;

        tracing::info!(
            model = %model,
            elapsed_ms = %start.elapsed().as_millis(),
            response_len = full_response.len(),
            "Ollama generate_with_images complete"
        );

        Ok(full_response)
    }

    /// R3: Chat-based vision inference via `/api/chat`.
    ///
    /// Required by chat-template models (MedGemma, LLaVA, Gemma) that expect
    /// messages with roles. The generate endpoint returns 500 for these models.
    ///
    /// Key behaviors (same as generate_with_images):
    /// - Image size guard: rejects images > 20 MB base64
    /// - Deterministic: temperature=0.0, num_predict=8192
    /// - keep_alive=30m: model stays loaded (caller must warm_model() first)
    /// - Streaming: uses `stream: true` to prevent OS idle socket timeout
    fn chat_with_images(
        &self,
        model: &str,
        user_prompt: &str,
        images: &[String],
        system: Option<&str>,
    ) -> Result<String, OllamaError> {
        validate_model_name(model)?;
        let _span = tracing::info_span!(
            "ollama_chat_with_images",
            model = %model,
            prompt_len = user_prompt.len(),
            image_count = images.len(),
        )
        .entered();
        let start = std::time::Instant::now();
        tracing::info!("Ollama chat_with_images starting (streaming)");

        // Guard: reject oversized images
        for (i, img) in images.iter().enumerate() {
            if img.len() > MAX_IMAGE_SIZE_BYTES {
                tracing::warn!(
                    image_index = i,
                    size = img.len(),
                    max = MAX_IMAGE_SIZE_BYTES,
                    "Image exceeds maximum size"
                );
                return Err(OllamaError::ImageTooLarge(img.len()));
            }
        }

        // Build messages array
        let mut messages = Vec::new();

        if let Some(sys) = system {
            messages.push(VisionChatMessage {
                role: "system".to_string(),
                content: sys.to_string(),
                images: None,
            });
        }

        messages.push(VisionChatMessage {
            role: "user".to_string(),
            content: user_prompt.to_string(),
            images: Some(images.to_vec()),
        });

        let request = VisionChatRequest {
            model: model.to_string(),
            messages,
            stream: true,
            options: Some(VisionGenerationOptions {
                temperature: 0.0,
                num_predict: 8192,
                num_ctx: self.vision_num_ctx,
            }),
            keep_alive: Some("30m".to_string()),
        };

        let url = format!("{}/api/chat", self.base_url);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .map_err(|e| {
                let err = if e.is_connect() || e.is_timeout() {
                    OllamaError::NotReachable
                } else {
                    OllamaError::Network(e.to_string())
                };
                tracing::warn!(
                    model = %model,
                    elapsed_ms = %start.elapsed().as_millis(),
                    error = %e,
                    cause = ?e.source(),
                    "Ollama chat_with_images failed"
                );
                err
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            tracing::warn!(
                model = %model,
                status = status.as_u16(),
                elapsed_ms = %start.elapsed().as_millis(),
                body = %body,
                "Ollama chat_with_images: non-success status"
            );
            return Err(OllamaError::ApiError {
                status: status.as_u16(),
                message: body,
            });
        }

        // Stream NDJSON — each line: { "message": { "content": "token" }, "done": false }
        // Data flows continuously, preventing OS idle socket timeout.
        let full_response = collect_chat_stream(response, model, &start)?;

        tracing::info!(
            model = %model,
            elapsed_ms = %start.elapsed().as_millis(),
            response_len = full_response.len(),
            "Ollama chat_with_images complete"
        );

        Ok(full_response)
    }
}

// ──────────────────────────────────────────────
// R3: Mock Vision Client (for testing)
// ──────────────────────────────────────────────

/// Mock vision client for testing — returns a configurable response.
///
/// Respects the image size guard to test error paths.
pub struct MockVisionClient {
    response: String,
    /// If true, simulates the image size check.
    enforce_size_limit: bool,
}

impl MockVisionClient {
    pub fn new(response: &str) -> Self {
        Self {
            response: response.to_string(),
            enforce_size_limit: true,
        }
    }

    pub fn without_size_limit(mut self) -> Self {
        self.enforce_size_limit = false;
        self
    }
}

impl VisionClient for MockVisionClient {
    fn generate_with_images(
        &self,
        _model: &str,
        _prompt: &str,
        images: &[String],
        _system: Option<&str>,
    ) -> Result<String, OllamaError> {
        if self.enforce_size_limit {
            for img in images {
                if img.len() > MAX_IMAGE_SIZE_BYTES {
                    return Err(OllamaError::ImageTooLarge(img.len()));
                }
            }
        }
        Ok(self.response.clone())
    }

    fn chat_with_images(
        &self,
        _model: &str,
        _user_prompt: &str,
        images: &[String],
        _system: Option<&str>,
    ) -> Result<String, OllamaError> {
        if self.enforce_size_limit {
            for img in images {
                if img.len() > MAX_IMAGE_SIZE_BYTES {
                    return Err(OllamaError::ImageTooLarge(img.len()));
                }
            }
        }
        Ok(self.response.clone())
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
    /// Model stays loaded for 30 minutes between requests.
    /// Prevents cold-start timeout during model loading on CPU.
    /// Model auto-unloads after idle period, freeing RAM.
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
    #[serde(skip_serializing_if = "Option::is_none")]
    num_ctx: Option<u32>,
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
                    num_ctx: self.options.num_ctx,
                },
                keep_alive: "30m",
            };

            let response = match self.client.post(&url)
                .json(&body)
                .send()
            {
                Ok(resp) => resp,
                Err(e) => {
                    if e.is_connect() || e.is_timeout() {
                        // Connection refused or connect timeout — Ollama is not running, don't retry
                        tracing::warn!(model = %model, error = %e, "Ollama generate: connection failed, not retrying");
                        return Err(StructuringError::OllamaConnection(self.base_url.clone()));
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

        let response = self.client.get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .map_err(|e| {
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
        let client = OllamaClient::new("http://localhost:11434");
        assert_eq!(client.base_url, "http://localhost:11434");
    }

    #[test]
    fn ollama_client_trims_trailing_slash() {
        let client = OllamaClient::new("http://localhost:11434/");
        assert_eq!(client.base_url, "http://localhost:11434");
    }

    #[test]
    fn default_local_uses_standard_port() {
        let client = OllamaClient::default_local();
        assert_eq!(client.base_url, "http://localhost:11434");
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
            num_ctx: Some(2048),
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
        let client = OllamaClient::new("http://localhost:99999");
        let (tx, _rx) = std::sync::mpsc::channel();
        let result = client.generate_streaming("../evil", "prompt", "system", tx);
        assert!(result.is_err());
    }

    #[test]
    fn generate_streaming_fails_when_unreachable() {
        let client = OllamaClient::new("http://localhost:99999");
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

    // ── R3: Vision client tests ──

    #[test]
    fn mock_vision_client_returns_configured_response() {
        let client = MockVisionClient::new("# Extracted Markdown\n\nPatient: John Doe");
        let images = vec!["base64data".to_string()];
        let result = client
            .generate_with_images("medgemma:4b", "Extract text", &images, None)
            .unwrap();
        assert!(result.contains("Extracted Markdown"));
    }

    #[test]
    fn mock_vision_client_rejects_oversized_image() {
        let client = MockVisionClient::new("ok");
        let huge_image = "x".repeat(MAX_IMAGE_SIZE_BYTES + 1);
        let images = vec![huge_image];
        let result = client.generate_with_images("model", "prompt", &images, None);
        assert!(matches!(result, Err(OllamaError::ImageTooLarge(_))));
    }

    #[test]
    fn mock_vision_client_accepts_image_at_limit() {
        let client = MockVisionClient::new("ok");
        let max_image = "x".repeat(MAX_IMAGE_SIZE_BYTES);
        let images = vec![max_image];
        let result = client.generate_with_images("model", "prompt", &images, None);
        assert!(result.is_ok());
    }

    #[test]
    fn mock_vision_client_bypasses_size_limit_when_configured() {
        let client = MockVisionClient::new("ok").without_size_limit();
        let huge_image = "x".repeat(MAX_IMAGE_SIZE_BYTES + 1);
        let images = vec![huge_image];
        let result = client.generate_with_images("model", "prompt", &images, None);
        assert!(result.is_ok());
    }

    #[test]
    fn vision_generate_rejects_invalid_model_name() {
        let client = OllamaClient::new("http://localhost:99999");
        let images = vec!["base64data".to_string()];
        let result = client.generate_with_images("../evil", "prompt", &images, None);
        assert!(matches!(result, Err(OllamaError::InvalidModelName(_))));
    }

    #[test]
    fn vision_generate_rejects_oversized_image() {
        let client = OllamaClient::new("http://localhost:99999");
        let huge_image = "x".repeat(MAX_IMAGE_SIZE_BYTES + 1);
        let images = vec![huge_image];
        let result =
            client.generate_with_images("medgemma:4b", "prompt", &images, None);
        assert!(matches!(result, Err(OllamaError::ImageTooLarge(_))));
    }

    #[test]
    fn vision_generate_fails_when_unreachable() {
        let client = OllamaClient::new("http://localhost:99999");
        let images = vec!["base64data".to_string()];
        let result =
            client.generate_with_images("medgemma:4b", "prompt", &images, None);
        assert!(result.is_err());
    }

    #[test]
    fn detect_capability_recognizes_medgemma() {
        let client = OllamaClient::new("http://localhost:99999");
        let cap = client.detect_capability("dcarrascosa/medgemma-1.5-4b-it").unwrap();
        assert_eq!(cap, ModelCapability::Vision);
    }

    #[test]
    fn detect_capability_recognizes_llava() {
        let client = OllamaClient::new("http://localhost:99999");
        let cap = client.detect_capability("llava:13b").unwrap();
        assert_eq!(cap, ModelCapability::Vision);
    }

    #[test]
    fn detect_capability_defaults_text_only_for_unknown() {
        // Unknown model + unreachable Ollama → conservative TextOnly
        let client = OllamaClient::new("http://localhost:99999");
        let cap = client.detect_capability("llama3:8b").unwrap();
        assert_eq!(cap, ModelCapability::TextOnly);
    }

    #[test]
    fn vision_max_image_size_reasonable() {
        assert_eq!(MAX_IMAGE_SIZE_BYTES, 20 * 1024 * 1024, "Max image size should be 20 MB");
    }

    // ── resolve_model tests (B10) ──
    // Note: These test the resolution LOGIC, not HTTP calls.
    // We test by checking error paths (no Ollama running in test env).

    #[test]
    fn resolve_model_returns_not_reachable_without_ollama() {
        let client = OllamaClient::new("http://localhost:99999");
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
