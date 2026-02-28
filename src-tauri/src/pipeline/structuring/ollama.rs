use serde::{Deserialize, Serialize};
use std::io::BufRead;

use super::StructuringError;
use super::ollama_types::{
    DoneReason, GenerationOptions, InferenceMetrics, ModelCapability, ModelDetail,
    ModelInfo, ModelPreference, OllamaError, OllamaHealth, OllamaShowResponse,
    OllamaTagsResponse, PullProgress, VisionChatMessage, VisionChatRequest,
    VisionGenerateRequest, VisionGenerationOptions, validate_base_url, validate_model_name,
};
use super::types::{LlmClient, VisionClient};

/// Raw HTTP response from vision POST — status + body bytes.
///
/// Intermediate type used by `send_vision_post()` to decouple HTTP transport
/// (async vs blocking) from NDJSON stream parsing.
struct VisionPostResponse {
    status: u16,
    body: Vec<u8>,
}

/// Ollama HTTP client for local LLM inference.
///
/// Completion-based operations — no artificial request timeouts.
/// Operations run until Ollama responds (success or error).
///
/// - `connect_timeout(10s)`: Fast detection of "Ollama not running"
/// - No `tcp_keepalive` — matches Ollama Go client (`http.DefaultClient`).
///   On Windows, `tcp_keepalive(N)` sets both keepalivetime AND keepaliveinterval
///   to N via `SIO_KEEPALIVE_VALS`, causing false timeouts at ~2N seconds when
///   TTFT exceeds the probe cycle (vision inference on CPU).
/// - Health/list/show: 5s per-request timeout (management operations)
/// - Delete: 30s per-request timeout (filesystem operation)
/// - Pull: no timeout (downloads are arbitrarily long)
/// - Generation/vision: no timeout (inference takes as long as needed)
///
/// SD-03: Blocking client for most operations. Tauri commands run on a threadpool.
///
/// Vision calls use the async client via `Handle::block_on()` when a tokio
/// runtime is available (Tauri app). On Windows, `reqwest::blocking::Client`
/// creates a single-threaded tokio runtime with IOCP that has a hard ~30s
/// limit — the async client bypasses this by reusing Tauri's multi-threaded
/// runtime. Falls back to blocking client in tests (no tokio runtime).
pub struct OllamaClient {
    base_url: String,
    /// Blocking HTTP client — connect_timeout only, no global request timeout.
    /// Used for all non-vision operations (health, list, show, generate text, etc.).
    client: reqwest::blocking::Client,
    /// Async HTTP client — same config as blocking, used for vision calls.
    /// Bypasses Windows IOCP ~30s timeout on `reqwest::blocking::Client`.
    async_client: reqwest::Client,
    /// Generation parameters (temperature, top_p, top_k, etc.).
    options: GenerationOptions,
    /// Context window for vision calls (hardware-tiered). None = model default.
    vision_num_ctx: Option<u32>,
    /// OLM-C1: Last inference metrics (updated after each generate/chat call).
    last_metrics: std::sync::Mutex<Option<InferenceMetrics>>,
    /// OLM-C5: Model keep-alive duration (hardware-tiered).
    /// Defaults to "30m". CPU vision uses "0" (unload to free RAM),
    /// CPU LLM uses "10m", GPU uses "30m".
    keep_alive: String,
}

// ──────────────────────────────────────────────
// BTL-05: Error classifiers — single source of truth
// ──────────────────────────────────────────────

/// Classify a reqwest send error into the correct `OllamaError` variant.
///
/// BTL-05: Replaces 14 inline `is_connect() || is_timeout() → NotReachable` patterns.
///
/// - Connect error → `NotReachable` (Ollama is actually not running)
/// - Timeout error → `ModelLoading` (model is cold-loading, connection timed out waiting)
/// - Other         → `Network` (generic transport error)
fn classify_send_error(e: &reqwest::Error, timeout_secs: Option<u64>) -> OllamaError {
    if e.is_connect() {
        OllamaError::NotReachable
    } else if e.is_timeout() {
        OllamaError::ModelLoading(timeout_secs.unwrap_or(30))
    } else {
        OllamaError::Network(e.to_string())
    }
}

// ──────────────────────────────────────────────
// OLM-C4: Structured error body parsing
//
// Ollama server returns errors as `{"error": "message"}` JSON.
// Our client was parsing response bodies as raw text, losing the
// structured message. This helper extracts the error message from
// JSON when available, falling back to the raw body text.
// ──────────────────────────────────────────────

/// Extract error message from Ollama error response body.
///
/// OLM-C4: Ollama server returns errors as `{"error": "message"}` JSON.
/// This helper tries to extract the message, falling back to the raw body.
fn parse_error_body(body: &str) -> String {
    #[derive(Deserialize)]
    struct OllamaErrorBody {
        error: String,
    }

    if let Ok(parsed) = serde_json::from_str::<OllamaErrorBody>(body) {
        parsed.error
    } else {
        body.to_string()
    }
}

// ──────────────────────────────────────────────
// OLLAMA_HOST parsing — mirrors Ollama's Go `envconfig.Host()`
//
// Source of truth: github.com/ollama/ollama/blob/main/envconfig/config.go
//
// OLLAMA_HOST is a **server bind** address that Ollama also uses as the
// client connection URL. When set to `0.0.0.0` (bind all interfaces),
// clients must rewrite to `127.0.0.1` because `0.0.0.0` is not a valid
// connect target on Windows. This is a known issue (ollama-python#407).
// ──────────────────────────────────────────────

/// Default Ollama port.
const OLLAMA_DEFAULT_PORT: u16 = 11434;

/// Parse `OLLAMA_HOST` into a client-connectable URL.
///
/// Mirrors Ollama's Go `envconfig.Host()` parsing logic:
/// 1. Split scheme from host:port (default scheme: `http`)
/// 2. Split host from port (default host: `127.0.0.1`, default port: `11434`)
/// 3. Rewrite bind-only `0.0.0.0` → `127.0.0.1` (not connectable on Windows)
///
/// Returns a full URL like `http://127.0.0.1:11434`.
fn parse_ollama_host(raw: &str) -> String {
    let s = raw.trim().trim_matches(|c| c == '"' || c == '\'');

    if s.is_empty() {
        return format!("http://127.0.0.1:{OLLAMA_DEFAULT_PORT}");
    }

    // Split scheme from hostport
    let (scheme, hostport) = if let Some((scheme, rest)) = s.split_once("://") {
        (scheme.to_string(), rest.to_string())
    } else {
        ("http".to_string(), s.to_string())
    };

    let default_port = match scheme.as_str() {
        "https" => 443u16,
        "http" if scheme == "http" => OLLAMA_DEFAULT_PORT,
        _ => OLLAMA_DEFAULT_PORT,
    };

    // Strip path (we only need host:port)
    let hostport = hostport.split('/').next().unwrap_or(&hostport);

    // Split host from port
    let (host, port) = parse_host_port(hostport, default_port);

    // Client-side fix: 0.0.0.0 is a bind address, not connectable on Windows
    let connect_host = if host == "0.0.0.0" { "127.0.0.1" } else { &host };

    format!("{scheme}://{connect_host}:{port}")
}

/// Split `host:port` handling IPv6 brackets.
///
/// Mirrors Go's `net.SplitHostPort` fallback logic from `envconfig.Host()`.
fn parse_host_port(hostport: &str, default_port: u16) -> (String, u16) {
    // IPv6 with brackets: [::1]:port
    if hostport.starts_with('[') {
        if let Some((bracketed, rest)) = hostport.split_once(']') {
            let host = bracketed.trim_start_matches('[').to_string();
            let port = rest
                .trim_start_matches(':')
                .parse::<u16>()
                .unwrap_or(default_port);
            return (host, port);
        }
    }

    // host:port — but only if exactly one colon (otherwise it's bare IPv6)
    if hostport.matches(':').count() == 1 {
        if let Some((host, port_str)) = hostport.rsplit_once(':') {
            if let Ok(port) = port_str.parse::<u16>() {
                let host = if host.is_empty() {
                    "127.0.0.1".to_string()
                } else {
                    host.to_string()
                };
                return (host, port);
            }
        }
    }

    // Bare host or bare IPv6 — use default port
    let host = if hostport.is_empty() {
        "127.0.0.1".to_string()
    } else {
        hostport.to_string()
    };
    (host, default_port)
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
        let user_agent = format!("coheara/{}", env!("CARGO_PKG_VERSION"));
        let connect_timeout = std::time::Duration::from_secs(10);

        let client = reqwest::blocking::Client::builder()
            .connect_timeout(connect_timeout)
            .user_agent(&user_agent) // OLM-C4
            .no_proxy()
            .build()
            .expect("Failed to create blocking HTTP client");

        let async_client = reqwest::Client::builder()
            .connect_timeout(connect_timeout)
            .user_agent(&user_agent) // OLM-C4
            .no_proxy()
            .build()
            .expect("Failed to create async HTTP client");

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
            async_client,
            options: GenerationOptions::default(),
            vision_num_ctx: None,
            last_metrics: std::sync::Mutex::new(None),
            keep_alive: "30m".to_string(),
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

    /// OLM-C5: Set model keep-alive duration (hardware-tiered).
    ///
    /// CPU vision: `"0"` (unload immediately to free RAM).
    /// CPU LLM: `"10m"`. GPU: `"30m"` (default).
    pub fn with_keep_alive(mut self, keep_alive: &str) -> Self {
        self.keep_alive = keep_alive.to_string();
        self
    }

    /// Set the context window size for vision calls (hardware-tiered).
    pub fn set_vision_num_ctx(&mut self, num_ctx: u32) {
        self.vision_num_ctx = Some(num_ctx);
    }

    /// OLM-C1: Get the last inference metrics (if available).
    ///
    /// Updated after each generate/chat call. Returns `None` if no inference
    /// has been performed or if the server didn't return metrics.
    pub fn last_metrics(&self) -> Option<InferenceMetrics> {
        self.last_metrics.lock().ok()?.clone()
    }

    /// OLM-C1: Store inference metrics internally.
    fn store_metrics(&self, metrics: Option<InferenceMetrics>) {
        if let Ok(mut guard) = self.last_metrics.lock() {
            *guard = metrics;
        }
    }

    /// Send a POST request for vision inference, returning the raw response body.
    ///
    /// Uses the async `reqwest::Client` via `handle.spawn()` + sync channel
    /// Send a POST request via the async client when a tokio runtime is
    /// available (Tauri app context). This bypasses the Windows IOCP ~30s
    /// hard timeout on `reqwest::blocking::Client`.
    ///
    /// Cannot use `Handle::block_on()` inside `spawn_blocking` — tokio detects
    /// the re-entrant runtime context and deadlocks. Instead, spawn an async
    /// task on the runtime's worker threads and wait via `mpsc::sync_channel`.
    ///
    /// Falls back to the blocking client when no tokio runtime is available
    /// (unit tests, CLI tools).
    ///
    /// Used for ALL inference POST calls (vision, generate, warm) — not just
    /// vision. The blocking client deadlocks inside Tauri's async runtime
    /// (reqwest#1215) when the response takes >30s.
    fn send_async_post<T: Serialize>(
        &self,
        url: &str,
        request: &T,
    ) -> Result<VisionPostResponse, OllamaError> {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                // Async path: spawn on Tauri's multi-threaded tokio runtime.
                // The sync channel bridges async→blocking without re-entering
                // the runtime context (which would deadlock).
                let async_client = self.async_client.clone();
                let url = url.to_string();
                let body = serde_json::to_vec(request)
                    .map_err(|e| OllamaError::Network(format!("Request serialization: {e}")))?;

                tracing::info!(url = %url, body_len = body.len(), "async_post: spawning task");

                let (tx, rx) = std::sync::mpsc::sync_channel(1);

                handle.spawn(async move {
                    let t0 = std::time::Instant::now();
                    tracing::info!(url = %url, body_len = body.len(), "async_post: sending request");

                    let result = async {
                        let resp = async_client
                            .post(&url)
                            .header(reqwest::header::CONTENT_TYPE, "application/json")
                            .body(body)
                            .send()
                            .await
                            .map_err(|e| {
                                tracing::warn!(url = %url, error = %e, elapsed_ms = %t0.elapsed().as_millis(), "async_post: send failed");
                                if e.is_connect() {
                                    OllamaError::NotReachable
                                } else {
                                    OllamaError::Network(e.to_string())
                                }
                            })?;

                        let status = resp.status();
                        let content_length = resp.content_length();
                        tracing::info!(url = %url, status = %status, content_length = ?content_length, elapsed_ms = %t0.elapsed().as_millis(), "async_post: got response headers");

                        tracing::info!(url = %url, "async_post: reading body");
                        let bytes = resp.bytes().await.map_err(|e| {
                            tracing::warn!(url = %url, error = %e, elapsed_ms = %t0.elapsed().as_millis(), "async_post: body read failed");
                            OllamaError::Network(format!("Response read: {e}"))
                        })?;

                        tracing::info!(url = %url, body_len = bytes.len(), elapsed_ms = %t0.elapsed().as_millis(), "async_post: body complete");
                        Ok::<_, OllamaError>(VisionPostResponse {
                            status: status.as_u16(),
                            body: bytes.to_vec(),
                        })
                    }
                    .await;

                    tracing::info!(url = %url, ok = result.is_ok(), elapsed_ms = %t0.elapsed().as_millis(), "async_post: result sent to channel");
                    let _ = tx.send(result);
                });

                rx.recv().map_err(|_| {
                    tracing::warn!("async_post: channel recv failed — task dropped");
                    OllamaError::Network(
                        "Async inference task dropped unexpectedly".to_string(),
                    )
                })?
            }
            Err(_) => {
                // Blocking fallback: no tokio runtime (tests, CLI).
                let resp = self.client.post(url).json(request).send().map_err(|e| {
                    classify_send_error(&e, None)
                })?;
                let status = resp.status();
                let body = resp.bytes()
                    .map_err(|e| OllamaError::Network(format!("Response read: {e}")))?
                    .to_vec();
                Ok(VisionPostResponse {
                    status: status.as_u16(),
                    body,
                })
            }
        }
    }

    /// Default Ollama instance at 127.0.0.1:11434.
    ///
    /// Uses explicit IPv4 loopback instead of `localhost` because Windows
    /// can resolve `localhost` to `[::1]` (IPv6). If Ollama binds IPv4-only,
    /// the IPv6 connection fails silently.
    pub fn default_local() -> Self {
        Self::new("http://127.0.0.1:11434")
    }

    /// Create an OllamaClient using the best available Ollama endpoint.
    ///
    /// Create an OllamaClient from the `OLLAMA_HOST` environment variable.
    ///
    /// Mirrors Ollama's Go `envconfig.Host()` parsing:
    /// - Unset/empty → `http://127.0.0.1:11434`
    /// - `0.0.0.0:11434` → `http://127.0.0.1:11434` (bind→connect rewrite)
    /// - `localhost:11434` → `http://localhost:11434`
    /// - `http://192.168.1.5:11434` → rejected (localhost-only policy)
    ///
    /// Works on Windows, Linux, and macOS. The `0.0.0.0` rewrite fixes
    /// the known issue where `OLLAMA_HOST=0.0.0.0` is not connectable
    /// on Windows (ollama-python#407).
    pub fn from_env() -> Self {
        let raw = std::env::var("OLLAMA_HOST").unwrap_or_default();
        let url = parse_ollama_host(&raw);

        // Validate localhost-only policy (security)
        if let Err(e) = validate_base_url(&url) {
            tracing::warn!(
                ollama_host = %raw,
                parsed_url = %url,
                error = %e,
                "OLLAMA_HOST rejected by localhost-only policy, using default"
            );
            return Self::new(&format!("http://127.0.0.1:{OLLAMA_DEFAULT_PORT}"));
        }

        tracing::debug!(ollama_host = %raw, url = %url, "Ollama client configured");
        Self::new(&url)
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
        // Include num_ctx so Ollama loads with the correct KvSize upfront.
        // Without this, warm loads with model default (e.g. 8192) but inference
        // sends num_ctx:4096 → Ollama unloads and reloads.
        let mut body = serde_json::json!({
            "model": model,
            "keep_alive": &self.keep_alive,  // OLM-C5: hardware-tiered
            "stream": false  // OLM-C4: explicit stream:false — matches Go client warm behavior
        });
        if let Some(num_ctx) = self.vision_num_ctx.or(self.options.num_ctx) {
            body["options"] = serde_json::json!({ "num_ctx": num_ctx });
        }

        let raw = self.send_async_post(&url, &body).map_err(|e| {
            tracing::warn!(
                model = %model,
                elapsed_ms = %start.elapsed().as_millis(),
                error = %e,
                "Model pre-warm failed"
            );
            e
        })?;

        if raw.status != 200 {
            let err_body = parse_error_body(&String::from_utf8_lossy(&raw.body));
            tracing::warn!(
                model = %model,
                elapsed_ms = %start.elapsed().as_millis(),
                body = %err_body,
                "Model pre-warm: non-success status"
            );
            return Err(OllamaError::ApiError {
                status: raw.status,
                message: err_body,
            });
        }

        tracing::info!(
            model = %model,
            elapsed_ms = %start.elapsed().as_millis(),
            "Model pre-warmed successfully"
        );

        Ok(())
    }

    /// BTL-06: Pre-warm a model via `/api/chat` (empty messages).
    ///
    /// Ollama uses separate code paths for `/api/generate` and `/api/chat`.
    /// Vision OCR uses `/api/chat` — warming via `/api/generate` does NOT
    /// warm the chat code path. This prevents cold-start timeouts on WSL2.
    pub fn warm_model_chat(&self, model: &str) -> Result<(), OllamaError> {
        validate_model_name(model)?;
        let _span = tracing::info_span!("ollama_warm_model_chat", model = %model).entered();
        let start = std::time::Instant::now();
        tracing::info!("Pre-warming model via /api/chat");

        let url = format!("{}/api/chat", self.base_url);
        // Include num_ctx so Ollama loads the model with the correct KvSize upfront.
        // Without this, warm loads with model default (e.g. 8192) but inference
        // sends num_ctx:4096 → Ollama unloads and reloads → 500 error.
        let mut body = serde_json::json!({
            "model": model,
            "messages": [],
            "keep_alive": &self.keep_alive,  // OLM-C5: hardware-tiered
            "stream": false  // OLM-C4: explicit stream:false — matches Go client warm behavior
        });
        if let Some(num_ctx) = self.vision_num_ctx.or(self.options.num_ctx) {
            body["options"] = serde_json::json!({ "num_ctx": num_ctx });
        }

        let raw = self.send_async_post(&url, &body).map_err(|e| {
            tracing::warn!(
                model = %model,
                elapsed_ms = %start.elapsed().as_millis(),
                error = %e,
                "Model pre-warm (chat) failed"
            );
            e
        })?;

        if raw.status != 200 {
            let err_body = parse_error_body(&String::from_utf8_lossy(&raw.body));
            tracing::warn!(
                model = %model,
                elapsed_ms = %start.elapsed().as_millis(),
                body = %err_body,
                "Model pre-warm (chat): non-success status"
            );
            return Err(OllamaError::ApiError {
                status: raw.status,
                message: err_body,
            });
        }

        tracing::info!(
            model = %model,
            elapsed_ms = %start.elapsed().as_millis(),
            "Model pre-warmed via /api/chat successfully"
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

        let _raw = self.send_async_post(&url, &body).map_err(|e| {
            tracing::warn!(model = %model, error = %e, "Model unload failed");
            e
        })?;

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

        // OLM-C4: HEAD / is lighter than GET / for periodic polling.
        // Ollama returns 200 OK if running. No body needed for health check.
        let url = format!("{}/", self.base_url);
        let response = self.client.head(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .map_err(|e| {
                let err = classify_send_error(&e, Some(5));
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

        // HEAD returns no body — version detection deferred to /api/version if needed
        let version = None;

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
                let err = classify_send_error(&e, Some(5));
                tracing::warn!(elapsed_ms = %start.elapsed().as_millis(), error = %e, "Ollama list_running failed");
                err
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = parse_error_body(&response.text().unwrap_or_default());
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
            /// OLM-C3: When this model will be unloaded (ISO 8601).
            #[serde(default)]
            expires_at: Option<String>,
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
                expires_at: m.expires_at,
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
                let err = classify_send_error(&e, Some(5));
                tracing::warn!(elapsed_ms = %start.elapsed().as_millis(), error = %e, "Ollama list_models failed");
                err
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = parse_error_body(&response.text().unwrap_or_default());
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
                let err = classify_send_error(&e, Some(5));
                tracing::warn!(model = %name, elapsed_ms = %start.elapsed().as_millis(), error = %e, "Ollama show_model failed");
                err
            })?;

        let status = response.status();
        if !status.is_success() {
            tracing::warn!(model = %name, status = status.as_u16(), "Ollama show_model: non-success status");
            if status.as_u16() == 404 {
                return Err(OllamaError::ModelNotFound(name.to_string()));
            }
            let body = parse_error_body(&response.text().unwrap_or_default());
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
            capabilities: parsed.capabilities, // OLM-C2
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
            let body = parse_error_body(&response.text().unwrap_or_default());
            return Err(OllamaError::PullFailed(format!(
                "HTTP {}: {body}",
                status.as_u16()
            )));
        }

        // Stream NDJSON — each line is a PullProgress JSON object
        let reader = std::io::BufReader::with_capacity(1_048_576, response);
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
                let err = classify_send_error(&e, Some(30));
                tracing::warn!(model = %name, elapsed_ms = %start.elapsed().as_millis(), error = %e, "Ollama delete_model failed");
                err
            })?;

        let status = response.status();
        if !status.is_success() {
            tracing::warn!(model = %name, status = status.as_u16(), "Ollama delete_model: non-success status");
            if status.as_u16() == 404 {
                return Err(OllamaError::ModelNotFound(name.to_string()));
            }
            let body = parse_error_body(&response.text().unwrap_or_default());
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
            "keep_alive": &self.keep_alive,  // OLM-C5: hardware-tiered
            "options": options,
        });

        let raw = self.send_async_post(&url, &body).map_err(|e| match &e {
            OllamaError::NotReachable => StructuringError::OllamaConnection(self.base_url.clone()),
            OllamaError::ModelLoading(_) => StructuringError::HttpClient("Model loading — request timed out".to_string()),
            _ => StructuringError::HttpClient(e.to_string()),
        })?;

        if raw.status != 200 {
            let body_text = parse_error_body(&String::from_utf8_lossy(&raw.body));
            return Err(StructuringError::OllamaError {
                status: raw.status as u16,
                body: body_text,
            });
        }

        // Parse collected NDJSON body — each line contains a partial response token
        let reader = std::io::BufReader::with_capacity(1_048_576, std::io::Cursor::new(raw.body));
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
                // OLM-C1: Present only on final chunk
                #[serde(default)]
                done_reason: Option<String>,
                #[serde(default)]
                total_duration: Option<u64>,
                #[serde(default)]
                load_duration: Option<u64>,
                #[serde(default)]
                prompt_eval_count: Option<u32>,
                #[serde(default)]
                prompt_eval_duration: Option<u64>,
                #[serde(default)]
                eval_count: Option<u32>,
                #[serde(default)]
                eval_duration: Option<u64>,
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
                        // OLM-C1: Capture metrics from final chunk (log only, no error for streaming)
                        let metrics = build_metrics_from_chunk(
                            chunk.done_reason.as_deref(),
                            chunk.total_duration,
                            chunk.load_duration,
                            chunk.prompt_eval_count,
                            chunk.prompt_eval_duration,
                            chunk.eval_count,
                            chunk.eval_duration,
                        );
                        log_truncation_warning(model, &metrics);
                        self.store_metrics(Some(metrics));
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
            "keep_alive": &self.keep_alive,  // OLM-C5: hardware-tiered
            "options": options,
        });

        let raw = self.send_async_post(&url, &body).map_err(|e| match &e {
            OllamaError::NotReachable => StructuringError::OllamaConnection(self.base_url.clone()),
            OllamaError::ModelLoading(_) => StructuringError::HttpClient("Model loading — request timed out".to_string()),
            _ => StructuringError::HttpClient(e.to_string()),
        })?;

        if raw.status != 200 {
            let body_text = parse_error_body(&String::from_utf8_lossy(&raw.body));
            return Err(StructuringError::OllamaError {
                status: raw.status as u16,
                body: body_text,
            });
        }

        // Parse collected NDJSON body with StreamGuard monitoring
        let reader = std::io::BufReader::with_capacity(1_048_576, std::io::Cursor::new(raw.body));
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
                // OLM-C1: Present only on final chunk
                #[serde(default)]
                done_reason: Option<String>,
                #[serde(default)]
                total_duration: Option<u64>,
                #[serde(default)]
                load_duration: Option<u64>,
                #[serde(default)]
                prompt_eval_count: Option<u32>,
                #[serde(default)]
                prompt_eval_duration: Option<u64>,
                #[serde(default)]
                eval_count: Option<u32>,
                #[serde(default)]
                eval_duration: Option<u64>,
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
                        // OLM-C1: Capture metrics from final chunk (log only for streaming)
                        let metrics = build_metrics_from_chunk(
                            chunk.done_reason.as_deref(),
                            chunk.total_duration,
                            chunk.load_duration,
                            chunk.prompt_eval_count,
                            chunk.prompt_eval_duration,
                            chunk.eval_count,
                            chunk.eval_duration,
                        );
                        log_truncation_warning(model, &metrics);
                        self.store_metrics(Some(metrics));
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
    /// OLM-C2: Checks `capabilities` array first (Ollama >= 0.6), falls back
    /// to keyword search in modelfile for older versions.
    /// CT-01 tags are the routing authority — this is a runtime probe only.
    ///
    /// Conservative: returns `TextOnly` if detection fails (e.g., Ollama unreachable).
    pub fn detect_capability(&self, model_name: &str) -> Result<ModelCapability, OllamaError> {
        match self.show_model(model_name) {
            Ok(detail) => {
                // OLM-C2: Check capabilities array first (Ollama >= 0.6)
                if detail.capabilities.iter().any(|c| c == "vision") {
                    tracing::debug!(model = %model_name, "Vision capability detected via capabilities array");
                    return Ok(ModelCapability::Vision);
                }

                // Fallback: keyword search in modelfile (Ollama < 0.6 compatibility)
                if let Some(ref modelfile) = detail.modelfile {
                    let lower = modelfile.to_lowercase();
                    if lower.contains("projector")
                        || lower.contains("mmproj")
                        || lower.contains("vision")
                    {
                        tracing::debug!(model = %model_name, "Vision capability detected via modelfile keywords (legacy)");
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
// OLM-C1: Metrics extraction from streaming chunks
// ──────────────────────────────────────────────

/// Parse `done_reason` string into `DoneReason` enum.
fn parse_done_reason(s: &str) -> Option<DoneReason> {
    serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
}

/// Build `InferenceMetrics` from the final chunk's fields.
fn build_metrics_from_chunk(
    done_reason: Option<&str>,
    total_duration: Option<u64>,
    load_duration: Option<u64>,
    prompt_eval_count: Option<u32>,
    prompt_eval_duration: Option<u64>,
    eval_count: Option<u32>,
    eval_duration: Option<u64>,
) -> InferenceMetrics {
    InferenceMetrics {
        done_reason: done_reason.and_then(parse_done_reason),
        total_duration_ns: total_duration,
        load_duration_ns: load_duration,
        prompt_eval_count,
        prompt_eval_duration_ns: prompt_eval_duration,
        eval_count,
        eval_duration_ns: eval_duration,
    }
}

/// Log a truncation warning if `done_reason` is "length".
fn log_truncation_warning(model: &str, metrics: &InferenceMetrics) {
    if metrics.is_truncated() {
        tracing::warn!(
            model = %model,
            eval_count = ?metrics.eval_count,
            "OLM-C1: Output TRUNCATED at num_predict limit — extraction may be incomplete"
        );
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
/// Final line: `{ "response": "", "done": true, "done_reason": "stop", ... }`
///
/// OLM-C1: Returns metrics from the final chunk alongside the text.
fn collect_generate_stream(
    reader: impl std::io::Read,
    model: &str,
    start: &std::time::Instant,
) -> Result<(String, Option<InferenceMetrics>), OllamaError> {
    #[derive(Deserialize)]
    struct GenerateChunk {
        response: String,
        #[serde(default)]
        done: bool,
        // OLM-C1: Present only on final chunk (done: true)
        #[serde(default)]
        done_reason: Option<String>,
        #[serde(default)]
        total_duration: Option<u64>,
        #[serde(default)]
        load_duration: Option<u64>,
        #[serde(default)]
        prompt_eval_count: Option<u32>,
        #[serde(default)]
        prompt_eval_duration: Option<u64>,
        #[serde(default)]
        eval_count: Option<u32>,
        #[serde(default)]
        eval_duration: Option<u64>,
    }

    let buf_reader = std::io::BufReader::with_capacity(1_048_576, reader);
    let mut full_response = String::new();
    let mut metrics = None;

    for line_result in buf_reader.lines() {
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
                    let m = build_metrics_from_chunk(
                        chunk.done_reason.as_deref(),
                        chunk.total_duration,
                        chunk.load_duration,
                        chunk.prompt_eval_count,
                        chunk.prompt_eval_duration,
                        chunk.eval_count,
                        chunk.eval_duration,
                    );
                    log_truncation_warning(model, &m);
                    metrics = Some(m);
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

    Ok((full_response, metrics))
}

/// SGV-01: Collect a streaming `/api/chat` response with StreamGuard monitoring.
///
/// Identical to `collect_chat_stream` but feeds each token through a `StreamGuard`
/// to detect degeneration (repetition loops in thinking tokens or output).
///
/// Returns `OllamaError::VisionDegeneration` if the guard detects a repetition pattern,
/// otherwise returns the full response string and metrics.
fn collect_chat_stream_guarded(
    reader: impl std::io::Read,
    model: &str,
    start: &std::time::Instant,
    guard_config: crate::pipeline::stream_guard::StreamGuardConfig,
) -> Result<(String, Option<InferenceMetrics>), OllamaError> {
    use crate::pipeline::stream_guard::StreamGuard;

    #[derive(Deserialize)]
    struct ChatChunk {
        message: Option<ChatChunkMessage>,
        #[serde(default)]
        done: bool,
        #[serde(default)]
        done_reason: Option<String>,
        #[serde(default)]
        total_duration: Option<u64>,
        #[serde(default)]
        load_duration: Option<u64>,
        #[serde(default)]
        prompt_eval_count: Option<u32>,
        #[serde(default)]
        prompt_eval_duration: Option<u64>,
        #[serde(default)]
        eval_count: Option<u32>,
        #[serde(default)]
        eval_duration: Option<u64>,
    }

    #[derive(Deserialize)]
    struct ChatChunkMessage {
        content: String,
    }

    let buf_reader = std::io::BufReader::with_capacity(1_048_576, reader);
    let mut guard = StreamGuard::new(guard_config);
    let mut metrics = None;

    for line_result in buf_reader.lines() {
        let line = line_result.map_err(|e| {
            tracing::warn!(
                model = %model,
                elapsed_ms = %start.elapsed().as_millis(),
                error = %e,
                "Stream read error during guarded vision chat"
            );
            OllamaError::Network(format!("Stream read error: {e}"))
        })?;

        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<ChatChunk>(&line) {
            Ok(chunk) => {
                if let Some(msg) = &chunk.message {
                    // Feed token to StreamGuard before accumulating
                    if let Err(abort) = guard.feed(&msg.content) {
                        tracing::warn!(
                            model = %model,
                            pattern = %abort.pattern,
                            tokens = abort.tokens_before_abort,
                            elapsed_ms = %start.elapsed().as_millis(),
                            "SGV-01: StreamGuard detected vision degeneration — aborting"
                        );
                        return Err(OllamaError::VisionDegeneration {
                            pattern: abort.pattern.to_string(),
                            tokens_before_abort: abort.tokens_before_abort,
                            partial_output: abort.partial_output,
                        });
                    }
                }
                if chunk.done {
                    let m = build_metrics_from_chunk(
                        chunk.done_reason.as_deref(),
                        chunk.total_duration,
                        chunk.load_duration,
                        chunk.prompt_eval_count,
                        chunk.prompt_eval_duration,
                        chunk.eval_count,
                        chunk.eval_duration,
                    );
                    log_truncation_warning(model, &m);
                    metrics = Some(m);
                    break;
                }
            }
            Err(e) => {
                tracing::warn!(
                    line = %line,
                    error = %e,
                    "Unparseable NDJSON line during guarded vision chat"
                );
            }
        }
    }

    tracing::info!(
        model = %model,
        elapsed_ms = %start.elapsed().as_millis(),
        tokens = guard.total_tokens(),
        "SGV-01: Guarded vision chat complete"
    );

    Ok((guard.accumulated_output().to_string(), metrics))
}

/// Collect a streaming `/api/chat` response into a single string.
///
/// Each NDJSON line: `{ "message": { "content": "token" }, "done": false }`
/// Final line: `{ "message": { "content": "" }, "done": true, "done_reason": "stop", ... }`
///
/// OLM-C1: Returns metrics from the final chunk alongside the text.
/// SGV-01: Production path now uses `collect_chat_stream_guarded()`.
/// This unguarded version is retained for parser unit tests.
#[cfg(test)]
fn collect_chat_stream(
    reader: impl std::io::Read,
    model: &str,
    start: &std::time::Instant,
) -> Result<(String, Option<InferenceMetrics>), OllamaError> {
    #[derive(Deserialize)]
    struct ChatChunk {
        message: Option<ChatChunkMessage>,
        #[serde(default)]
        done: bool,
        // OLM-C1: Present only on final chunk (done: true)
        #[serde(default)]
        done_reason: Option<String>,
        #[serde(default)]
        total_duration: Option<u64>,
        #[serde(default)]
        load_duration: Option<u64>,
        #[serde(default)]
        prompt_eval_count: Option<u32>,
        #[serde(default)]
        prompt_eval_duration: Option<u64>,
        #[serde(default)]
        eval_count: Option<u32>,
        #[serde(default)]
        eval_duration: Option<u64>,
    }

    #[derive(Deserialize)]
    struct ChatChunkMessage {
        content: String,
    }

    let buf_reader = std::io::BufReader::with_capacity(1_048_576, reader);
    let mut full_response = String::new();
    let mut metrics = None;

    for line_result in buf_reader.lines() {
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
                    let m = build_metrics_from_chunk(
                        chunk.done_reason.as_deref(),
                        chunk.total_duration,
                        chunk.load_duration,
                        chunk.prompt_eval_count,
                        chunk.prompt_eval_duration,
                        chunk.eval_count,
                        chunk.eval_duration,
                    );
                    log_truncation_warning(model, &m);
                    metrics = Some(m);
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

    Ok((full_response, metrics))
}

impl VisionClient for OllamaClient {
    /// Generate text from a prompt with base64-encoded images via Ollama `/api/generate`.
    ///
    /// R4: Used for document OCR (MedGemma default) and medical image interpretation.
    ///
    /// Key behaviors:
    /// - Image size guard: rejects images > 20 MB base64
    /// - Deterministic: temperature=0.0, num_predict=8192
    /// - keep_alive: hardware-tiered (caller must warm_model() first)
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
            keep_alive: Some(self.keep_alive.clone()),  // OLM-C5: hardware-tiered
        };

        let url = format!("{}/api/generate", self.base_url);

        // Use async client to bypass Windows IOCP ~30s timeout on blocking client.
        // Vision TTFT can exceed 48s on CPU/Vulkan GPU.
        let raw = self.send_async_post(&url, &request).map_err(|e| {
            tracing::warn!(
                model = %model,
                elapsed_ms = %start.elapsed().as_millis(),
                error = %e,
                "Ollama generate_with_images failed"
            );
            e
        })?;

        if raw.status < 200 || raw.status >= 300 {
            let body = parse_error_body(&String::from_utf8_lossy(&raw.body));
            tracing::warn!(
                model = %model,
                status = raw.status,
                elapsed_ms = %start.elapsed().as_millis(),
                body = %body,
                "Ollama generate_with_images: non-success status"
            );
            return Err(OllamaError::ApiError {
                status: raw.status,
                message: body,
            });
        }

        // Parse NDJSON from collected response body.
        let cursor = std::io::Cursor::new(raw.body);
        let (full_response, metrics) = collect_generate_stream(cursor, model, &start)?;

        // OLM-C1: Store metrics for later retrieval
        self.store_metrics(metrics);

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
    /// - keep_alive: hardware-tiered (caller must warm_model() first)
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
            keep_alive: Some(self.keep_alive.clone()),  // OLM-C5: hardware-tiered
        };

        let url = format!("{}/api/chat", self.base_url);

        // Use async client to bypass Windows IOCP ~30s timeout on blocking client.
        // Vision TTFT can exceed 48s on CPU/Vulkan GPU.
        let raw = self.send_async_post(&url, &request).map_err(|e| {
            tracing::warn!(
                model = %model,
                elapsed_ms = %start.elapsed().as_millis(),
                error = %e,
                "Ollama chat_with_images failed"
            );
            e
        })?;

        if raw.status < 200 || raw.status >= 300 {
            let body = parse_error_body(&String::from_utf8_lossy(&raw.body));
            tracing::warn!(
                model = %model,
                status = raw.status,
                elapsed_ms = %start.elapsed().as_millis(),
                body = %body,
                "Ollama chat_with_images: non-success status"
            );
            return Err(OllamaError::ApiError {
                status: raw.status,
                message: body,
            });
        }

        // SGV-01: Parse NDJSON with StreamGuard monitoring for degeneration.
        // Vision OCR on constrained SLMs can enter thinking-token repetition loops
        // (e.g., "Titre" × 200) that consume the entire output budget.
        let cursor = std::io::Cursor::new(raw.body);
        let guard_config = crate::pipeline::stream_guard::StreamGuardConfig::default();
        let (full_response, metrics) =
            collect_chat_stream_guarded(cursor, model, &start, guard_config)?;

        // OLM-C1: Store metrics for later retrieval
        self.store_metrics(metrics);

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
    /// OLM-C5: Hardware-tiered keep-alive (CPU="0"/"10m", GPU="30m").
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
    // OLM-C1: Metrics from non-streaming generate
    #[serde(default)]
    done_reason: Option<String>,
    #[serde(default)]
    total_duration: Option<u64>,
    #[serde(default)]
    load_duration: Option<u64>,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    prompt_eval_duration: Option<u64>,
    #[serde(default)]
    eval_count: Option<u32>,
    #[serde(default)]
    eval_duration: Option<u64>,
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
                keep_alive: &self.keep_alive,  // OLM-C5: hardware-tiered
            };

            let raw = match self.send_async_post(&url, &body) {
                Ok(r) => r,
                Err(e) => {
                    let classified = match &e {
                        OllamaError::NotReachable => StructuringError::OllamaConnection(self.base_url.clone()),
                        OllamaError::ModelLoading(_) => StructuringError::HttpClient("Model loading — request timed out".to_string()),
                        _ => StructuringError::HttpClient(e.to_string()),
                    };
                    if matches!(e, OllamaError::NotReachable | OllamaError::ModelLoading(_)) {
                        tracing::warn!(model = %model, error = %e, "Ollama generate: connection/timeout, not retrying");
                        return Err(classified);
                    }
                    last_error = Some(classified);
                    continue;
                }
            };

            if raw.status == 200 {
                let parsed: OllamaGenerateResponse = serde_json::from_slice(&raw.body)
                    .map_err(|e| StructuringError::ResponseParsing(e.to_string()))?;

                // OLM-C1: Extract and store metrics from non-streaming response
                let metrics = build_metrics_from_chunk(
                    parsed.done_reason.as_deref(),
                    parsed.total_duration,
                    parsed.load_duration,
                    parsed.prompt_eval_count,
                    parsed.prompt_eval_duration,
                    parsed.eval_count,
                    parsed.eval_duration,
                );
                log_truncation_warning(model, &metrics);
                self.store_metrics(Some(metrics));

                tracing::info!(
                    model = %model,
                    elapsed_ms = %start.elapsed().as_millis(),
                    response_len = parsed.response.len(),
                    attempts = attempt + 1,
                    "Ollama generate complete"
                );

                return Ok(parsed.response);
            }

            let status_code = raw.status;
            let body_text = parse_error_body(&String::from_utf8_lossy(&raw.body));

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
            let body = parse_error_body(&response.text().unwrap_or_default());
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
    fn default_local_uses_ipv4_loopback() {
        let client = OllamaClient::default_local();
        assert_eq!(client.base_url, "http://127.0.0.1:11434");
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
        assert_eq!(client.base_url(), "http://127.0.0.1:11434");
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

    // ── OLLAMA_HOST parsing tests ──
    // Tests parse_ollama_host() directly — mirrors Ollama's Go envconfig.Host().

    #[test]
    fn parse_host_empty_returns_default() {
        assert_eq!(parse_ollama_host(""), "http://127.0.0.1:11434");
    }

    #[test]
    fn parse_host_bare_host_port() {
        assert_eq!(
            parse_ollama_host("localhost:11434"),
            "http://localhost:11434"
        );
    }

    #[test]
    fn parse_host_bare_ip_port() {
        assert_eq!(
            parse_ollama_host("127.0.0.1:11434"),
            "http://127.0.0.1:11434"
        );
    }

    #[test]
    fn parse_host_full_url() {
        assert_eq!(
            parse_ollama_host("http://127.0.0.1:11434"),
            "http://127.0.0.1:11434"
        );
    }

    #[test]
    fn parse_host_zero_addr_rewritten() {
        // 0.0.0.0 is a bind address — rewrite to 127.0.0.1 for connect
        assert_eq!(
            parse_ollama_host("0.0.0.0:11434"),
            "http://127.0.0.1:11434"
        );
    }

    #[test]
    fn parse_host_zero_addr_with_scheme_rewritten() {
        assert_eq!(
            parse_ollama_host("http://0.0.0.0:11434"),
            "http://127.0.0.1:11434"
        );
    }

    #[test]
    fn parse_host_bare_hostname_gets_default_port() {
        assert_eq!(
            parse_ollama_host("localhost"),
            "http://localhost:11434"
        );
    }

    #[test]
    fn parse_host_custom_port() {
        assert_eq!(
            parse_ollama_host("localhost:9999"),
            "http://localhost:9999"
        );
    }

    #[test]
    fn parse_host_strips_quotes_and_whitespace() {
        assert_eq!(
            parse_ollama_host("  \"localhost:11434\"  "),
            "http://localhost:11434"
        );
    }

    #[test]
    fn parse_host_ipv6_loopback() {
        assert_eq!(
            parse_ollama_host("[::1]:11434"),
            "http://::1:11434"
        );
    }

    // ── from_env / URL validation tests ──

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
        let client = OllamaClient::from_env();
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

    // BTL-02: detect_capability no longer uses prefix heuristic.
    // CT-01 tags are the sole routing authority. detect_capability is a
    // runtime probe via /api/show — unreachable Ollama → TextOnly.

    #[test]
    fn detect_capability_defaults_text_only_when_unreachable() {
        // Unreachable Ollama → conservative TextOnly (no prefix heuristic)
        let client = OllamaClient::new("http://localhost:99999");
        let cap = client.detect_capability("dcarrascosa/medgemma-1.5-4b-it").unwrap();
        assert_eq!(cap, ModelCapability::TextOnly);
    }

    #[test]
    fn detect_capability_text_only_for_any_model_when_unreachable() {
        let client = OllamaClient::new("http://localhost:99999");
        let cap = client.detect_capability("llava:13b").unwrap();
        assert_eq!(cap, ModelCapability::TextOnly);
    }

    // ── BTL-06: warm_model_chat ──

    #[test]
    fn warm_model_chat_fails_on_unreachable() {
        let client = OllamaClient::new("http://localhost:99999");
        let result = client.warm_model_chat("medgemma:4b");
        assert!(result.is_err());
    }

    #[test]
    fn warm_model_chat_rejects_invalid_name() {
        let client = OllamaClient::new("http://localhost:99999");
        let result = client.warm_model_chat("");
        assert!(result.is_err());
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

    // ── OLM-C1: Chunk deserialization with metrics ──

    #[test]
    fn generate_chunk_with_metrics_deserializes() {
        // Final chunk from /api/generate includes metrics
        let json = r#"{
            "model": "medgemma:4b",
            "response": "",
            "done": true,
            "done_reason": "stop",
            "total_duration": 5000000000,
            "load_duration": 100000000,
            "prompt_eval_count": 10,
            "prompt_eval_duration": 500000000,
            "eval_count": 100,
            "eval_duration": 2000000000
        }"#;

        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct GenerateChunk {
            response: String,
            #[serde(default)]
            done: bool,
            #[serde(default)]
            done_reason: Option<String>,
            #[serde(default)]
            total_duration: Option<u64>,
            #[serde(default)]
            load_duration: Option<u64>,
            #[serde(default)]
            prompt_eval_count: Option<u32>,
            #[serde(default)]
            prompt_eval_duration: Option<u64>,
            #[serde(default)]
            eval_count: Option<u32>,
            #[serde(default)]
            eval_duration: Option<u64>,
        }

        let chunk: GenerateChunk = serde_json::from_str(json).unwrap();
        assert!(chunk.done);
        assert_eq!(chunk.done_reason.as_deref(), Some("stop"));
        assert_eq!(chunk.eval_count, Some(100));
        assert_eq!(chunk.eval_duration, Some(2_000_000_000));

        let metrics = build_metrics_from_chunk(
            chunk.done_reason.as_deref(),
            chunk.total_duration,
            chunk.load_duration,
            chunk.prompt_eval_count,
            chunk.prompt_eval_duration,
            chunk.eval_count,
            chunk.eval_duration,
        );
        assert!(!metrics.is_truncated());
        let tps = metrics.tokens_per_second().unwrap();
        assert!((tps - 50.0).abs() < 0.01, "Expected 50 tok/s, got {tps}");
    }

    #[test]
    fn chat_chunk_with_metrics_deserializes() {
        let json = r#"{
            "message": {"role": "assistant", "content": ""},
            "done": true,
            "done_reason": "length",
            "total_duration": 3000000000,
            "load_duration": 0,
            "prompt_eval_count": 20,
            "prompt_eval_duration": 400000000,
            "eval_count": 8192,
            "eval_duration": 2500000000
        }"#;

        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct ChatChunk {
            message: Option<ChatChunkMsg>,
            #[serde(default)]
            done: bool,
            #[serde(default)]
            done_reason: Option<String>,
            #[serde(default)]
            eval_count: Option<u32>,
            #[serde(default)]
            total_duration: Option<u64>,
            #[serde(default)]
            load_duration: Option<u64>,
            #[serde(default)]
            prompt_eval_count: Option<u32>,
            #[serde(default)]
            prompt_eval_duration: Option<u64>,
            #[serde(default)]
            eval_duration: Option<u64>,
        }

        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct ChatChunkMsg {
            content: String,
        }

        let chunk: ChatChunk = serde_json::from_str(json).unwrap();
        assert!(chunk.done);
        assert_eq!(chunk.done_reason.as_deref(), Some("length"));
        assert_eq!(chunk.eval_count, Some(8192));

        let metrics = build_metrics_from_chunk(
            chunk.done_reason.as_deref(),
            chunk.total_duration,
            chunk.load_duration,
            chunk.prompt_eval_count,
            chunk.prompt_eval_duration,
            chunk.eval_count,
            chunk.eval_duration,
        );
        assert!(metrics.is_truncated(), "length reason should be truncated");
    }

    #[test]
    fn non_streaming_response_with_metrics_deserializes() {
        let json = r#"{
            "model": "medgemma:4b",
            "response": "OK",
            "done": true,
            "done_reason": "stop",
            "total_duration": 1000000000,
            "load_duration": 50000000,
            "prompt_eval_count": 5,
            "prompt_eval_duration": 200000000,
            "eval_count": 2,
            "eval_duration": 100000000
        }"#;

        let parsed: OllamaGenerateResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.response, "OK");
        assert_eq!(parsed.done_reason.as_deref(), Some("stop"));
        assert_eq!(parsed.eval_count, Some(2));
    }

    #[test]
    fn parse_done_reason_all_variants() {
        assert_eq!(parse_done_reason("stop"), Some(DoneReason::Stop));
        assert_eq!(parse_done_reason("length"), Some(DoneReason::Length));
        assert_eq!(parse_done_reason("load"), Some(DoneReason::Load));
        assert_eq!(parse_done_reason("unload"), Some(DoneReason::Unload));
        assert_eq!(parse_done_reason("unknown_value"), None);
        assert_eq!(parse_done_reason(""), None);
    }

    #[test]
    fn build_metrics_from_chunk_all_none() {
        let metrics = build_metrics_from_chunk(None, None, None, None, None, None, None);
        assert!(metrics.done_reason.is_none());
        assert!(!metrics.is_truncated());
        assert!(metrics.tokens_per_second().is_none());
    }

    #[test]
    fn last_metrics_initially_none() {
        let client = OllamaClient::new("http://localhost:99999");
        assert!(client.last_metrics().is_none());
    }

    // ── OLM-C4: HTTP Protocol Alignment ──

    #[test]
    fn parse_error_body_extracts_json_error() {
        let body = r#"{"error": "model 'foo' not found, try pulling it first"}"#;
        let msg = parse_error_body(body);
        assert_eq!(msg, "model 'foo' not found, try pulling it first");
    }

    #[test]
    fn parse_error_body_falls_back_to_raw() {
        let body = "Internal Server Error";
        let msg = parse_error_body(body);
        assert_eq!(msg, "Internal Server Error");
    }

    #[test]
    fn parse_error_body_handles_malformed_json() {
        let body = r#"{"error": }"#;
        let msg = parse_error_body(body);
        assert_eq!(msg, r#"{"error": }"#);
    }

    #[test]
    fn parse_error_body_handles_empty() {
        let msg = parse_error_body("");
        assert_eq!(msg, "");
    }

    #[test]
    fn parse_error_body_handles_json_without_error_field() {
        let body = r#"{"status": "failed", "message": "something"}"#;
        let msg = parse_error_body(body);
        // Falls back to raw body because no "error" field
        assert_eq!(msg, body);
    }

    #[test]
    fn user_agent_format() {
        // Verify the user agent string format matches "coheara/{version}"
        let ua = format!("coheara/{}", env!("CARGO_PKG_VERSION"));
        assert!(ua.starts_with("coheara/"), "User agent should start with coheara/");
        // Version should not be empty
        let version = ua.strip_prefix("coheara/").unwrap();
        assert!(!version.is_empty(), "Version should not be empty");
    }

    // ── OLM-C5: Hardware-Tiered keep_alive ──

    #[test]
    fn default_keep_alive_is_30m() {
        let client = OllamaClient::new("http://localhost:99999");
        assert_eq!(client.keep_alive, "30m");
    }

    #[test]
    fn with_keep_alive_overrides_default() {
        let client = OllamaClient::new("http://localhost:99999")
            .with_keep_alive("10m");
        assert_eq!(client.keep_alive, "10m");
    }

    #[test]
    fn with_keep_alive_zero_for_cpu_vision() {
        let client = OllamaClient::new("http://localhost:99999")
            .with_keep_alive("0");
        assert_eq!(client.keep_alive, "0");
    }

    #[test]
    fn with_keep_alive_chains_with_options() {
        let options = GenerationOptions {
            temperature: 0.1,
            ..GenerationOptions::default()
        };
        let client = OllamaClient::new("http://localhost:99999")
            .with_options(options)
            .with_keep_alive("5m");
        assert_eq!(client.keep_alive, "5m");
        assert!((client.options.temperature - 0.1).abs() < f32::EPSILON);
    }

    // ──────────────────────────────────────────────
    // Async vision bypass tests
    // ──────────────────────────────────────────────

    #[test]
    fn collect_generate_stream_from_cursor() {
        let ndjson = r#"{"response":"Hello","done":false}
{"response":" world","done":false}
{"response":"","done":true,"done_reason":"stop","eval_count":10,"eval_duration":1000000000}
"#;
        let cursor = std::io::Cursor::new(ndjson.as_bytes());
        let start = std::time::Instant::now();
        let (text, metrics) = collect_generate_stream(cursor, "test-model", &start).unwrap();
        assert_eq!(text, "Hello world");
        let m = metrics.unwrap();
        assert_eq!(m.eval_count, Some(10));
        assert!(!m.is_truncated());
    }

    #[test]
    fn collect_chat_stream_from_cursor() {
        let ndjson = r#"{"message":{"content":"Hello"},"done":false}
{"message":{"content":" doc"},"done":false}
{"message":{"content":""},"done":true,"done_reason":"stop","eval_count":5,"eval_duration":500000000}
"#;
        let cursor = std::io::Cursor::new(ndjson.as_bytes());
        let start = std::time::Instant::now();
        let (text, metrics) = collect_chat_stream(cursor, "test-model", &start).unwrap();
        assert_eq!(text, "Hello doc");
        let m = metrics.unwrap();
        assert_eq!(m.eval_count, Some(5));
        assert!(!m.is_truncated());
    }

    #[test]
    fn collect_generate_stream_detects_truncation_from_cursor() {
        let ndjson = r#"{"response":"partial","done":true,"done_reason":"length","eval_count":8192}
"#;
        let cursor = std::io::Cursor::new(ndjson.as_bytes());
        let start = std::time::Instant::now();
        let (text, metrics) = collect_generate_stream(cursor, "test-model", &start).unwrap();
        assert_eq!(text, "partial");
        let m = metrics.unwrap();
        assert!(m.is_truncated());
    }

    #[test]
    fn collect_chat_stream_detects_truncation_from_cursor() {
        let ndjson = r#"{"message":{"content":"partial"},"done":true,"done_reason":"length","eval_count":8192}
"#;
        let cursor = std::io::Cursor::new(ndjson.as_bytes());
        let start = std::time::Instant::now();
        let (text, metrics) = collect_chat_stream(cursor, "test-model", &start).unwrap();
        assert_eq!(text, "partial");
        let m = metrics.unwrap();
        assert!(m.is_truncated());
    }

    #[test]
    fn vision_post_response_carries_status_and_body() {
        let resp = VisionPostResponse {
            status: 200,
            body: b"test body".to_vec(),
        };
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, b"test body");
    }

    #[test]
    fn vision_post_response_error_status() {
        let resp = VisionPostResponse {
            status: 500,
            body: br#"{"error":"model not found"}"#.to_vec(),
        };
        assert_eq!(resp.status, 500);
        let body = parse_error_body(&String::from_utf8_lossy(&resp.body));
        assert_eq!(body, "model not found");
    }

    #[test]
    fn async_client_built_in_constructor() {
        // Verify both clients are created without panic
        let client = OllamaClient::new("http://localhost:99999");
        // async_client exists (struct has the field)
        assert_eq!(client.base_url, "http://localhost:99999");
    }

    // ── SGV-01: Guarded chat stream tests ──────────────

    #[test]
    fn guarded_chat_stream_healthy_response() {
        let ndjson = r#"{"message":{"content":"Hello"},"done":false}
{"message":{"content":" world"},"done":false}
{"message":{"content":""},"done":true,"done_reason":"stop","eval_count":5,"eval_duration":500000000}
"#;
        let cursor = std::io::Cursor::new(ndjson.as_bytes());
        let start = std::time::Instant::now();
        let config = crate::pipeline::stream_guard::StreamGuardConfig::default();
        let (text, metrics) =
            collect_chat_stream_guarded(cursor, "test-model", &start, config).unwrap();
        assert_eq!(text, "Hello world");
        let m = metrics.unwrap();
        assert_eq!(m.eval_count, Some(5));
        assert!(!m.is_truncated());
    }

    #[test]
    fn guarded_chat_stream_detects_token_repeat() {
        // Simulate a degeneration pattern: same token repeated 25 times
        let mut lines = String::new();
        for _ in 0..25 {
            lines.push_str(r#"{"message":{"content":"Titre"},"done":false}"#);
            lines.push('\n');
        }
        lines.push_str(
            r#"{"message":{"content":""},"done":true,"done_reason":"stop"}"#,
        );
        lines.push('\n');

        let cursor = std::io::Cursor::new(lines.as_bytes());
        let start = std::time::Instant::now();
        let config = crate::pipeline::stream_guard::StreamGuardConfig {
            max_consecutive_identical: 20,
            ..Default::default()
        };
        let result = collect_chat_stream_guarded(cursor, "test-model", &start, config);
        assert!(result.is_err());
        match result.unwrap_err() {
            OllamaError::VisionDegeneration {
                pattern,
                tokens_before_abort,
                ..
            } => {
                assert!(pattern.contains("token_repeat"));
                assert!(tokens_before_abort >= 20);
            }
            other => panic!("Expected VisionDegeneration, got: {other:?}"),
        }
    }

    #[test]
    fn guarded_chat_stream_detects_sequence_repeat() {
        // Simulate a repeating 2-token sequence: "A" "B" repeated many times
        let mut lines = String::new();
        // 5 repeats of a 10-token sequence should trigger at default config
        for _ in 0..60 {
            lines.push_str(r#"{"message":{"content":"Header "},"done":false}"#);
            lines.push('\n');
            lines.push_str(r#"{"message":{"content":"Titre "},"done":false}"#);
            lines.push('\n');
        }
        lines.push_str(
            r#"{"message":{"content":""},"done":true,"done_reason":"stop"}"#,
        );
        lines.push('\n');

        let cursor = std::io::Cursor::new(lines.as_bytes());
        let start = std::time::Instant::now();
        let config = crate::pipeline::stream_guard::StreamGuardConfig {
            sequence_length: 2,
            max_sequence_repeats: 5,
            max_consecutive_identical: 100, // high to not trigger token repeat
            ..Default::default()
        };
        let result = collect_chat_stream_guarded(cursor, "test-model", &start, config);
        assert!(result.is_err());
        match result.unwrap_err() {
            OllamaError::VisionDegeneration { pattern, .. } => {
                assert!(pattern.contains("sequence_repeat"));
            }
            other => panic!("Expected VisionDegeneration, got: {other:?}"),
        }
    }

    #[test]
    fn guarded_chat_stream_respects_token_limit() {
        // Exceed max_total_tokens
        let mut lines = String::new();
        for i in 0..15 {
            lines.push_str(&format!(
                r#"{{"message":{{"content":"token{i}"}},"done":false}}"#
            ));
            lines.push('\n');
        }
        lines.push_str(
            r#"{"message":{"content":""},"done":true,"done_reason":"stop"}"#,
        );
        lines.push('\n');

        let cursor = std::io::Cursor::new(lines.as_bytes());
        let start = std::time::Instant::now();
        let config = crate::pipeline::stream_guard::StreamGuardConfig {
            max_total_tokens: 10,
            max_consecutive_identical: 100,
            max_sequence_repeats: 100,
            ..Default::default()
        };
        let result = collect_chat_stream_guarded(cursor, "test-model", &start, config);
        assert!(result.is_err());
        match result.unwrap_err() {
            OllamaError::VisionDegeneration { pattern, .. } => {
                assert!(pattern.contains("token_limit"));
            }
            other => panic!("Expected VisionDegeneration, got: {other:?}"),
        }
    }

    #[test]
    fn guarded_chat_stream_preserves_metrics() {
        let ndjson = r#"{"message":{"content":"ok"},"done":false}
{"message":{"content":""},"done":true,"done_reason":"length","eval_count":8192,"eval_duration":1000000000}
"#;
        let cursor = std::io::Cursor::new(ndjson.as_bytes());
        let start = std::time::Instant::now();
        let config = crate::pipeline::stream_guard::StreamGuardConfig::default();
        let (text, metrics) =
            collect_chat_stream_guarded(cursor, "test-model", &start, config).unwrap();
        assert_eq!(text, "ok");
        let m = metrics.unwrap();
        assert!(m.is_truncated());
        assert_eq!(m.eval_count, Some(8192));
    }
}
