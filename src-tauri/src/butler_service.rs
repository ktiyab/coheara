//! BTL-04: Butler Service — central SLM lifecycle orchestrator.
//!
//! Wraps `OllamaService` with model state tracking, warm-endpoint awareness,
//! and hardware caching. Industry precedent: Apple NSOperationQueue (lifecycle),
//! Google MediaSession (state machine), Signal JobManager (exclusivity).
//!
//! **Single responsibility**: Knows which model is loaded, which endpoints are
//! warm, and what hardware is available — so callers don't have to.
//!
//! **Backward compat**: `inner()` exposes the underlying `OllamaService` so
//! existing call sites can migrate incrementally (BTL-07).

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;

use crate::hardware::GpuTier;
use crate::ollama_service::{OllamaGuard, OllamaService, OllamaServiceError, OperationKind};
use crate::pipeline::structuring::ollama_types::{normalize_model_identity, InferenceMetrics};

// ═══════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════

/// Which Ollama endpoint has been warmed for the loaded model.
///
/// Ollama uses separate code paths for `/api/generate` (text completion)
/// and `/api/chat` (vision + multi-turn). Warming one does NOT warm the other.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WarmEndpoint {
    /// `/api/generate` — text completion (structuring, extraction).
    Generate,
    /// `/api/chat` — chat messages, vision OCR (multi-modal).
    Chat,
}

/// Warm state of a model for a specific endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WarmState {
    /// Model is loaded and this endpoint has been warmed.
    Hot,
    /// Model is loaded but this endpoint hasn't been warmed yet.
    Loaded,
    /// Different model or no model loaded.
    Cold,
}

// ═══════════════════════════════════════════════════════════
// ModelStateTracker — internal state machine
// ═══════════════════════════════════════════════════════════

/// Tracks which model is loaded and which endpoints are warm.
///
/// Protected by `Mutex` inside `ButlerService`. Updated on acquire/drop.
struct ModelStateTracker {
    /// Normalized name of the currently loaded model (`None` = no model loaded).
    loaded_model: Option<String>,
    /// Which endpoints have been warmed for the loaded model.
    warm_endpoints: HashSet<WarmEndpoint>,
    /// When the last operation completed (for idle tracking).
    last_activity: Option<Instant>,
    /// OLM-C3: Last inference metrics from Ollama (updated after each operation).
    last_metrics: Option<InferenceMetrics>,
    /// OLM-C3: When the loaded model will be unloaded (from `/api/ps`).
    model_expires_at: Option<String>,
}

impl ModelStateTracker {
    fn new() -> Self {
        Self {
            loaded_model: None,
            warm_endpoints: HashSet::new(),
            last_activity: None,
            last_metrics: None,
            model_expires_at: None,
        }
    }

    /// Update state when a new model is loaded.
    /// If it's a different model, warm endpoints are cleared.
    fn on_model_acquired(&mut self, model: &str) {
        let normalized = normalize_model_identity(model);
        if self.loaded_model.as_deref() != Some(&normalized) {
            // Different model — reset warm state
            self.warm_endpoints.clear();
            self.loaded_model = Some(normalized);
        }
    }

    /// Update state when an operation completes (guard dropped).
    fn on_operation_complete(&mut self) {
        self.last_activity = Some(Instant::now());
    }

    /// Mark an endpoint as warm for the current model.
    fn mark_warm(&mut self, endpoint: WarmEndpoint) {
        self.warm_endpoints.insert(endpoint);
    }

    /// Check if a model is loaded (normalized comparison).
    fn is_model_loaded(&self, model: &str) -> bool {
        let normalized = normalize_model_identity(model);
        self.loaded_model.as_deref() == Some(&normalized)
    }

    /// Check warm state for a model + endpoint combination.
    fn warm_state(&self, model: &str, endpoint: WarmEndpoint) -> WarmState {
        if !self.is_model_loaded(model) {
            return WarmState::Cold;
        }
        if self.warm_endpoints.contains(&endpoint) {
            WarmState::Hot
        } else {
            WarmState::Loaded
        }
    }

    /// OLM-C3: Record inference metrics from the last operation.
    fn record_metrics(&mut self, metrics: Option<InferenceMetrics>) {
        if let Some(m) = metrics {
            self.last_metrics = Some(m);
        }
    }

    /// OLM-C3: Record when the model will expire (from `/api/ps`).
    fn record_expires_at(&mut self, expires_at: Option<String>) {
        self.model_expires_at = expires_at;
    }
}

// ═══════════════════════════════════════════════════════════
// CachedHardware — avoid repeated /api/ps calls
// ═══════════════════════════════════════════════════════════

/// Cached hardware detection result.
///
/// Hardware doesn't change during a session — detect once, cache forever.
struct CachedHardware {
    tier: GpuTier,
    #[allow(dead_code)]
    detected_at: Instant,
}

// ═══════════════════════════════════════════════════════════
// ButlerService
// ═══════════════════════════════════════════════════════════

/// Central SLM lifecycle orchestrator.
///
/// Lives on `CoreState` (shared via `Arc`). Wraps `OllamaService` and adds:
/// - **Model state tracking**: Knows which model is loaded, avoids redundant warm.
/// - **Warm endpoint tracking**: `/api/generate` vs `/api/chat` are separate code paths.
/// - **Hardware caching**: `GpuTier` detected once per session.
///
/// Backward compat: `inner()` returns `&OllamaService` so existing code works
/// unchanged until migrated in BTL-07.
pub struct ButlerService {
    inner: OllamaService,
    state: Mutex<ModelStateTracker>,
    hardware: Mutex<Option<CachedHardware>>,
}

impl ButlerService {
    /// Create a new ButlerService with idle state.
    pub fn new() -> Self {
        Self {
            inner: OllamaService::new(),
            state: Mutex::new(ModelStateTracker::new()),
            hardware: Mutex::new(None),
        }
    }

    /// Access the underlying OllamaService (backward compat).
    ///
    /// Use this during incremental migration — new code should use
    /// Butler's own acquire/warm/status methods instead.
    pub fn inner(&self) -> &OllamaService {
        &self.inner
    }

    // ── Acquisition ──────────────────────────────────────────

    /// Acquire exclusive Ollama access and track the model.
    ///
    /// Blocks until Ollama is free. Updates model state tracker so
    /// subsequent `is_model_loaded` / `warm_state` queries reflect reality.
    pub fn acquire(
        &self,
        kind: OperationKind,
        model: &str,
    ) -> Result<ButlerGuard<'_>, OllamaServiceError> {
        let ollama_guard = self.inner.acquire(kind, model)?;

        // Update model state tracker
        if let Ok(mut tracker) = self.state.lock() {
            tracker.on_model_acquired(model);
        }

        Ok(ButlerGuard {
            _ollama_guard: ollama_guard,
            butler: self,
        })
    }

    /// Try to acquire without blocking. Returns `None` if Ollama is busy.
    ///
    /// Use for verification — a running operation IS verification.
    pub fn try_acquire(
        &self,
        kind: OperationKind,
        model: &str,
    ) -> Option<ButlerGuard<'_>> {
        let ollama_guard = self.inner.try_acquire(kind, model)?;

        if let Ok(mut tracker) = self.state.lock() {
            tracker.on_model_acquired(model);
        }

        Some(ButlerGuard {
            _ollama_guard: ollama_guard,
            butler: self,
        })
    }

    // ── Model state queries ──────────────────────────────────

    /// Check if a model is currently loaded (normalized comparison).
    pub fn is_model_loaded(&self, model: &str) -> bool {
        self.state
            .lock()
            .map(|tracker| tracker.is_model_loaded(model))
            .unwrap_or(false)
    }

    /// Check warm state for a model + endpoint combination.
    pub fn warm_state(&self, model: &str, endpoint: WarmEndpoint) -> WarmState {
        self.state
            .lock()
            .map(|tracker| tracker.warm_state(model, endpoint))
            .unwrap_or(WarmState::Cold)
    }

    /// Mark an endpoint as warm for the currently loaded model.
    ///
    /// Called after a successful warm request (generate or chat).
    pub fn mark_warm(&self, endpoint: WarmEndpoint) {
        if let Ok(mut tracker) = self.state.lock() {
            tracker.mark_warm(endpoint);
        }
    }

    /// Explicitly set the loaded model (e.g. after warm completes).
    pub fn mark_loaded(&self, model: &str) {
        if let Ok(mut tracker) = self.state.lock() {
            tracker.on_model_acquired(model);
        }
    }

    // ── Readiness protocol (BTL-06) ────────────────────────

    /// Ensure the model is ready on the given endpoint.
    ///
    /// Skips warming if already hot. Warms via the correct Ollama API
    /// (`/api/generate` for Generate, `/api/chat` for Chat).
    /// Updates warm state tracker on success.
    pub fn ensure_ready(
        &self,
        client: &crate::pipeline::structuring::ollama::OllamaClient,
        model: &str,
        endpoint: WarmEndpoint,
    ) -> Result<(), crate::pipeline::structuring::ollama_types::OllamaError> {
        if self.warm_state(model, endpoint) == WarmState::Hot {
            tracing::debug!(model = %model, endpoint = ?endpoint, "Already warm — skipping");
            return Ok(());
        }

        match endpoint {
            WarmEndpoint::Generate => client.warm_model(model)?,
            WarmEndpoint::Chat => client.warm_model_chat(model)?,
        }

        self.mark_loaded(model);
        self.mark_warm(endpoint);
        Ok(())
    }

    // ── Hardware caching ─────────────────────────────────────

    /// Get cached hardware tier (if detected).
    pub fn hardware_tier(&self) -> Option<GpuTier> {
        self.hardware
            .lock()
            .ok()
            .and_then(|hw| hw.as_ref().map(|c| c.tier))
    }

    /// Cache detected hardware tier. Subsequent calls are no-ops.
    pub fn cache_hardware(&self, tier: GpuTier) {
        if let Ok(mut hw) = self.hardware.lock() {
            if hw.is_none() {
                *hw = Some(CachedHardware {
                    tier,
                    detected_at: Instant::now(),
                });
            }
        }
    }

    // ── OLM-C3: Inference metrics ─────────────────────────────

    /// Record inference metrics from the last Ollama operation.
    ///
    /// OLM-C3: Call after each inference with `client.last_metrics()`.
    /// Enables measured vs estimated tok/s comparison.
    pub fn record_metrics(&self, metrics: Option<InferenceMetrics>) {
        if let Ok(mut tracker) = self.state.lock() {
            tracker.record_metrics(metrics);
        }
    }

    /// Get the measured tokens per second from the last inference.
    ///
    /// OLM-C3: Compare with `PipelineConfig.estimated_tok_per_sec`
    /// to detect hardware tier calibration drift.
    pub fn measured_tokens_per_second(&self) -> Option<f64> {
        self.state
            .lock()
            .ok()
            .and_then(|tracker| {
                tracker.last_metrics.as_ref().and_then(|m| m.tokens_per_second())
            })
    }

    /// Record when the loaded model will expire from Ollama's memory.
    ///
    /// OLM-C3: Retrieved from `/api/ps` response.
    pub fn record_model_expires_at(&self, expires_at: Option<String>) {
        if let Ok(mut tracker) = self.state.lock() {
            tracker.record_expires_at(expires_at);
        }
    }

    // ── Status snapshot ──────────────────────────────────────

    /// Snapshot of Butler state for frontend/debugging.
    pub fn status(&self) -> ButlerStatus {
        let (loaded_model, warm_endpoints, idle_secs, model_expires_at, measured_tok_per_sec) = self
            .state
            .lock()
            .map(|tracker| {
                let idle = tracker
                    .last_activity
                    .map(|t| t.elapsed().as_secs())
                    .unwrap_or(0);
                let warm: Vec<WarmEndpoint> =
                    tracker.warm_endpoints.iter().copied().collect();
                let tps = tracker.last_metrics.as_ref().and_then(|m| m.tokens_per_second());
                (tracker.loaded_model.clone(), warm, idle, tracker.model_expires_at.clone(), tps)
            })
            .unwrap_or_default();

        let hardware_tier = self.hardware_tier();
        let active_operation = self.inner.current_operation();

        ButlerStatus {
            loaded_model,
            warm_endpoints,
            hardware_tier,
            active_operation: active_operation.map(|op| op.kind.to_string()),
            idle_secs,
            model_expires_at,
            measured_tok_per_sec,
        }
    }
}

impl Default for ButlerService {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════
// ButlerGuard — RAII token with state tracking
// ═══════════════════════════════════════════════════════════

/// RAII guard for exclusive Ollama access with Butler state tracking.
///
/// Wraps `OllamaGuard` and updates `ModelStateTracker` on drop.
pub struct ButlerGuard<'a> {
    _ollama_guard: OllamaGuard<'a>,
    butler: &'a ButlerService,
}

impl Drop for ButlerGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut tracker) = self.butler.state.lock() {
            tracker.on_operation_complete();
        }
    }
}

// ═══════════════════════════════════════════════════════════
// ButlerSession — L6-11: enforced SLM call boundary
// ═══════════════════════════════════════════════════════════

use crate::pipeline::domain_contracts::DomainContract;
use crate::pipeline::quality_gate::{
    self, QualityGateConfig, QualityGateFailure,
};
use crate::pipeline::safety::output_sanitize::sanitize_llm_output;
use crate::pipeline::strategy::{
    self, ContextType, PromptStrategy,
};
use crate::pipeline::stream_guard::StreamGuardConfig;
use crate::pipeline::structuring::ollama_types::OllamaError;
use crate::pipeline::structuring::types::{LlmClient, VisionCallParams, VisionClient};

// ═══════════════════════════════════════════════════════════
// ValidatedOutput — C4: enforced output from ButlerSession
// ═══════════════════════════════════════════════════════════

/// Output that has passed through all defense layers:
/// sanitization (C2) → quality gate (C3).
///
/// Callers receiving `ValidatedOutput` are guaranteed clean data.
#[derive(Debug, Clone)]
pub struct ValidatedOutput {
    /// Sanitized, quality-checked text.
    pub text: String,
    /// Approximate token count (whitespace-split words).
    pub tokens_generated: usize,
    /// Model that produced this output.
    pub model: String,
}

// ═══════════════════════════════════════════════════════════
// SessionError — C4: defense-aware error from ButlerSession
// ═══════════════════════════════════════════════════════════

/// Errors from ButlerSession LLM calls, with defense context.
///
/// Each variant maps to a specific defense layer failure:
/// - `Degeneration`: StreamGuard (C1) detected repetition mid-stream
/// - `QualityGate`: Quality gate (C3) rejected the completed output
/// - `Llm`: Underlying network/API error
/// - `RetriesExhausted`: All retry attempts failed
#[derive(Debug)]
pub enum SessionError {
    /// Underlying LLM call failed (network, API, model not found).
    Llm(String),
    /// StreamGuard detected degeneration in the token stream.
    Degeneration {
        pattern: String,
        tokens_before_abort: usize,
        partial_output: String,
    },
    /// Quality gate rejected the output (low diversity, line dominance).
    QualityGate {
        reason: String,
        raw_output: String,
    },
    /// All retry attempts exhausted.
    RetriesExhausted {
        attempts: u32,
        last_error: Box<SessionError>,
    },
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Llm(msg) => write!(f, "LLM error: {msg}"),
            Self::Degeneration { pattern, tokens_before_abort, .. } => {
                write!(f, "Degeneration detected ({pattern}) after {tokens_before_abort} tokens")
            }
            Self::QualityGate { reason, .. } => write!(f, "Quality gate rejected: {reason}"),
            Self::RetriesExhausted { attempts, last_error } => {
                write!(f, "All {attempts} retries exhausted: {last_error}")
            }
        }
    }
}

impl std::error::Error for SessionError {}

impl From<OllamaServiceError> for SessionError {
    fn from(e: OllamaServiceError) -> Self {
        Self::Llm(e.to_string())
    }
}

/// Enforced call boundary for all SLM interaction.
///
/// Wraps `ButlerGuard` (exclusive access) with resolved configuration:
/// `PromptStrategy`, `StreamGuardConfig`, `QualityGateConfig`, and optional
/// `DomainContract`. Callers declare `ContextType` at creation, Butler
/// resolves everything.
///
/// **Configuration resolver + output processor**:
/// 1. Resolves strategy from (`ContextType`, `ModelVariant`)
/// 2. Derives `StreamGuardConfig` from strategy
/// 3. Provides output post-processing (sanitize → quality gate)
/// 4. Holds optional `DomainContract` for prompt generation
///
/// Industry precedent: Apple `AVCaptureSession` (configure → startRunning),
/// Android `CameraDevice.createCaptureSession` (preset → pipeline).
pub struct ButlerSession<'a> {
    /// RAII guard — held for exclusive access, released on drop.
    #[allow(dead_code)]
    guard: ButlerGuard<'a>,
    model: String,
    context: ContextType,
    strategy: PromptStrategy,
    guard_config: StreamGuardConfig,
    quality_config: QualityGateConfig,
    contract: Option<&'static DomainContract>,
}

/// Derive `StreamGuardConfig` from a resolved `PromptStrategy`.
///
/// The strategy's `max_tokens` caps the guard's total token limit.
/// All other guard parameters use calibrated defaults (BM-04/05/06).
fn derive_guard_config(strategy: &PromptStrategy) -> StreamGuardConfig {
    StreamGuardConfig {
        max_total_tokens: strategy.max_tokens as usize,
        ..StreamGuardConfig::default()
    }
}

impl<'a> ButlerSession<'a> {
    // ── Accessors ──────────────────────────────────────────

    /// The resolved prompt strategy for this session.
    pub fn strategy(&self) -> &PromptStrategy {
        &self.strategy
    }

    /// The stream guard configuration derived from the strategy.
    pub fn stream_guard_config(&self) -> &StreamGuardConfig {
        &self.guard_config
    }

    /// The quality gate configuration for this session.
    pub fn quality_config(&self) -> &QualityGateConfig {
        &self.quality_config
    }

    /// The optional domain contract bound to this session.
    pub fn contract(&self) -> Option<&'static DomainContract> {
        self.contract
    }

    /// The calling context type for this session.
    pub fn context(&self) -> ContextType {
        self.context
    }

    /// The model name for this session.
    pub fn model(&self) -> &str {
        &self.model
    }

    // ── Builder methods ────────────────────────────────────

    /// Bind a domain contract to this session (for prompt generation).
    pub fn with_contract(mut self, contract: &'static DomainContract) -> Self {
        self.contract = Some(contract);
        self
    }

    /// Override the quality gate configuration (default is fine for most uses).
    pub fn with_quality_config(mut self, config: QualityGateConfig) -> Self {
        self.quality_config = config;
        self
    }

    // ── Output processing ──────────────────────────────────

    /// Strip thinking tokens and model artifacts from raw LLM output.
    pub fn sanitize(&self, raw: &str) -> String {
        sanitize_llm_output(raw)
    }

    /// Check if output passes the quality gate (diversity + line dominance).
    pub fn validate_output(&self, text: &str) -> Result<(), QualityGateFailure> {
        quality_gate::validate_output(text, &self.quality_config)
    }

    /// Sanitize output, then validate it through the quality gate.
    ///
    /// Returns the sanitized text on success, or the quality failure on error.
    /// This is the standard output processing pipeline:
    /// raw → sanitize → quality gate → clean text.
    pub fn process_output(&self, raw: &str) -> Result<String, QualityGateFailure> {
        let sanitized = self.sanitize(raw);
        self.validate_output(&sanitized)?;
        Ok(sanitized)
    }

    // ── LLM call methods (C4: enforced boundary) ──────────

    /// Text generation through session with defense enforcement.
    ///
    /// Pipeline: client.generate() → sanitize (C2) → quality gate (C3) → ValidatedOutput.
    /// Maps StructuringError::Degeneration to SessionError::Degeneration.
    pub fn generate(
        &self,
        client: &dyn LlmClient,
        prompt: &str,
        system: &str,
    ) -> Result<ValidatedOutput, SessionError> {
        let raw = client
            .generate(&self.model, prompt, system)
            .map_err(|e| match e {
                crate::pipeline::structuring::StructuringError::Degeneration {
                    pattern,
                    tokens_before_abort,
                    partial_output,
                } => SessionError::Degeneration {
                    pattern,
                    tokens_before_abort,
                    partial_output,
                },
                other => SessionError::Llm(other.to_string()),
            })?;

        let clean = self.process_output(&raw).map_err(|failure| {
            SessionError::QualityGate {
                reason: failure.to_string(),
                raw_output: raw.clone(),
            }
        })?;

        Ok(ValidatedOutput {
            tokens_generated: clean.split_whitespace().count(),
            text: clean,
            model: self.model.clone(),
        })
    }

    /// Vision generation through session with defense enforcement.
    ///
    /// Pipeline: client.chat_with_images() → sanitize (C2) → quality gate (C3) → ValidatedOutput.
    /// Maps OllamaError::VisionDegeneration to SessionError::Degeneration.
    pub fn chat_with_images(
        &self,
        client: &dyn VisionClient,
        user_prompt: &str,
        images: &[String],
        system: Option<&str>,
    ) -> Result<ValidatedOutput, SessionError> {
        let params = VisionCallParams {
            temperature: Some(self.strategy.temperature),
            num_predict: Some(self.strategy.max_tokens as i32),
        };
        let raw = client
            .chat_with_images_with_params(&self.model, user_prompt, images, system, params)
            .map_err(|e| match e {
                OllamaError::VisionDegeneration {
                    pattern,
                    tokens_before_abort,
                    partial_output,
                } => SessionError::Degeneration {
                    pattern,
                    tokens_before_abort,
                    partial_output,
                },
                other => SessionError::Llm(other.to_string()),
            })?;

        let clean = self.process_output(&raw).map_err(|failure| {
            SessionError::QualityGate {
                reason: failure.to_string(),
                raw_output: raw.clone(),
            }
        })?;

        Ok(ValidatedOutput {
            tokens_generated: clean.split_whitespace().count(),
            text: clean,
            model: self.model.clone(),
        })
    }
}

// ═══════════════════════════════════════════════════════════
// VisionSession — C4: trait for session-like vision callers
// ═══════════════════════════════════════════════════════════

/// Trait for objects that can make validated vision LLM calls.
///
/// Implemented by `ButlerSession` (guarded, production) and
/// `FallbackSession` (unguarded, for C4 fallback when caller
/// already holds the butler guard).
pub trait VisionSession: Send + Sync {
    /// Make a vision LLM call with full defense pipeline.
    fn chat_with_images(
        &self,
        client: &dyn VisionClient,
        user_prompt: &str,
        images: &[String],
        system: Option<&str>,
    ) -> Result<ValidatedOutput, SessionError>;

    /// Model name for this session.
    fn model(&self) -> &str;
}

/// C4: Lightweight session for fallback — no guard, no lifetime.
///
/// Used when the caller already holds the butler guard (e.g., import_queue_worker).
/// Replicates ButlerSession's defense pipeline (sanitize → quality gate) without
/// requiring exclusive Ollama access (the caller's guard provides that).
pub struct FallbackSession {
    model: String,
    strategy: PromptStrategy,
    quality_config: QualityGateConfig,
}

impl FallbackSession {
    /// Create from context + model (same resolution as ButlerSession).
    pub fn new(model: &str, context: ContextType, has_gpu: bool) -> Self {
        let variant = strategy::detect_model_variant(model);
        let strategy = strategy::resolve_strategy_with_gpu(context, variant, has_gpu);
        Self {
            model: model.to_string(),
            strategy,
            quality_config: QualityGateConfig::default(),
        }
    }
}

impl VisionSession for FallbackSession {
    fn chat_with_images(
        &self,
        client: &dyn VisionClient,
        user_prompt: &str,
        images: &[String],
        system: Option<&str>,
    ) -> Result<ValidatedOutput, SessionError> {
        let params = VisionCallParams {
            temperature: Some(self.strategy.temperature),
            num_predict: Some(self.strategy.max_tokens as i32),
        };
        let raw = client
            .chat_with_images_with_params(&self.model, user_prompt, images, system, params)
            .map_err(|e| match e {
                OllamaError::VisionDegeneration {
                    pattern,
                    tokens_before_abort,
                    partial_output,
                } => SessionError::Degeneration {
                    pattern,
                    tokens_before_abort,
                    partial_output,
                },
                other => SessionError::Llm(other.to_string()),
            })?;

        let sanitized = sanitize_llm_output(&raw);
        quality_gate::validate_output(&sanitized, &self.quality_config).map_err(|failure| {
            SessionError::QualityGate {
                reason: failure.to_string(),
                raw_output: raw.clone(),
            }
        })?;

        Ok(ValidatedOutput {
            tokens_generated: sanitized.split_whitespace().count(),
            text: sanitized,
            model: self.model.clone(),
        })
    }

    fn model(&self) -> &str {
        &self.model
    }
}

impl ButlerService {
    /// Start a new SLM session with resolved configuration.
    ///
    /// Acquires exclusive Ollama access, resolves the prompt strategy from
    /// the calling context + model variant, and derives the stream guard
    /// config. The returned session provides output post-processing methods.
    ///
    /// # Arguments
    /// * `kind` — Operation kind for Ollama lock tracking
    /// * `model` — Model name (variant auto-detected)
    /// * `context` — Calling context that determines strategy
    pub fn start_session(
        &self,
        kind: OperationKind,
        model: &str,
        context: ContextType,
    ) -> Result<ButlerSession<'_>, OllamaServiceError> {
        let guard = self.acquire(kind, model)?;
        let variant = strategy::detect_model_variant(model);
        let gpu = self.hardware_tier().map_or(false, |t| {
            matches!(t, GpuTier::FullGpu | GpuTier::PartialGpu)
        });
        let strategy = strategy::resolve_strategy_with_gpu(context, variant, gpu);
        let guard_config = derive_guard_config(&strategy);

        Ok(ButlerSession {
            guard,
            model: model.to_string(),
            context,
            strategy,
            guard_config,
            quality_config: QualityGateConfig::default(),
            contract: None,
        })
    }
}

// ═══════════════════════════════════════════════════════════
// ButlerStatus — serializable snapshot
// ═══════════════════════════════════════════════════════════

/// Serializable snapshot of Butler state (for IPC / frontend).
#[derive(Debug, Clone, Serialize)]
pub struct ButlerStatus {
    /// Normalized name of the currently loaded model.
    pub loaded_model: Option<String>,
    /// Which endpoints are warm.
    pub warm_endpoints: Vec<WarmEndpoint>,
    /// Cached hardware tier.
    pub hardware_tier: Option<GpuTier>,
    /// Description of active operation (if any).
    pub active_operation: Option<String>,
    /// Seconds since last operation completed.
    pub idle_secs: u64,
    /// OLM-C3: When the loaded model will be unloaded (ISO 8601).
    pub model_expires_at: Option<String>,
    /// OLM-C3: Measured tokens per second from last inference.
    pub measured_tok_per_sec: Option<f64>,
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_butler_is_idle() {
        let butler = ButlerService::new();
        assert!(!butler.is_model_loaded("medgemma:4b"));
        assert_eq!(butler.warm_state("medgemma:4b", WarmEndpoint::Generate), WarmState::Cold);
        assert!(butler.hardware_tier().is_none());
        assert!(!butler.inner().is_busy());
    }

    #[test]
    fn acquire_tracks_loaded_model() {
        let butler = ButlerService::new();
        let _guard = butler.acquire(OperationKind::DocumentOcr, "medgemma:4b").unwrap();
        assert!(butler.is_model_loaded("medgemma:4b"));
        assert!(butler.inner().is_busy());
    }

    #[test]
    fn acquire_normalizes_model_identity() {
        let butler = ButlerService::new();
        let _guard = butler.acquire(OperationKind::DocumentOcr, "ktiyab/coheara-medgemma-4b-f16:latest").unwrap();
        // Normalized form should match
        assert!(butler.is_model_loaded("coheara-medgemma-4b-f16"));
        assert!(butler.is_model_loaded("ktiyab/coheara-medgemma-4b-f16:latest"));
    }

    #[test]
    fn model_change_clears_warm_state() {
        let butler = ButlerService::new();
        {
            let _guard = butler.acquire(OperationKind::DocumentOcr, "medgemma:4b").unwrap();
            butler.mark_warm(WarmEndpoint::Generate);
            assert_eq!(butler.warm_state("medgemma:4b", WarmEndpoint::Generate), WarmState::Hot);
        }
        // Same model — still warm (model didn't change)
        assert_eq!(butler.warm_state("medgemma:4b", WarmEndpoint::Generate), WarmState::Hot);

        // Load different model — previous model's warm state cleared
        {
            let _guard = butler.acquire(OperationKind::ChatGeneration, "llama3:8b").unwrap();
            // medgemma is no longer loaded → Cold
            assert_eq!(butler.warm_state("medgemma:4b", WarmEndpoint::Generate), WarmState::Cold);
            // llama3 is loaded but not warmed → Loaded
            assert_eq!(butler.warm_state("llama3:8b", WarmEndpoint::Generate), WarmState::Loaded);
        }
    }

    #[test]
    fn warm_state_cold_loaded_hot_progression() {
        let butler = ButlerService::new();

        // Cold: no model loaded
        assert_eq!(butler.warm_state("medgemma:4b", WarmEndpoint::Chat), WarmState::Cold);

        // Loaded: model acquired but endpoint not warmed
        let _guard = butler.acquire(OperationKind::DocumentOcr, "medgemma:4b").unwrap();
        assert_eq!(butler.warm_state("medgemma:4b", WarmEndpoint::Chat), WarmState::Loaded);

        // Hot: endpoint warmed
        butler.mark_warm(WarmEndpoint::Chat);
        assert_eq!(butler.warm_state("medgemma:4b", WarmEndpoint::Chat), WarmState::Hot);

        // Generate is still only Loaded (not warmed)
        assert_eq!(butler.warm_state("medgemma:4b", WarmEndpoint::Generate), WarmState::Loaded);
    }

    #[test]
    fn try_acquire_returns_none_when_busy() {
        let butler = ButlerService::new();
        let _guard = butler.acquire(OperationKind::DocumentOcr, "medgemma:4b").unwrap();
        assert!(butler.try_acquire(OperationKind::ChatGeneration, "medgemma:4b").is_none());
    }

    #[test]
    fn try_acquire_tracks_model() {
        let butler = ButlerService::new();
        let guard = butler.try_acquire(OperationKind::ModelVerification, "medgemma:4b");
        assert!(guard.is_some());
        assert!(butler.is_model_loaded("medgemma:4b"));
    }

    #[test]
    fn hardware_caching() {
        let butler = ButlerService::new();
        assert!(butler.hardware_tier().is_none());

        butler.cache_hardware(GpuTier::FullGpu);
        assert_eq!(butler.hardware_tier(), Some(GpuTier::FullGpu));

        // Subsequent cache is no-op (first detection wins)
        butler.cache_hardware(GpuTier::CpuOnly);
        assert_eq!(butler.hardware_tier(), Some(GpuTier::FullGpu));
    }

    #[test]
    fn guard_drop_records_activity() {
        let butler = ButlerService::new();
        {
            let _guard = butler.acquire(OperationKind::DocumentOcr, "medgemma:4b").unwrap();
        }
        // After guard drop, last_activity should be set
        let status = butler.status();
        // idle_secs should be very small (just dropped)
        assert!(status.idle_secs < 2);
    }

    #[test]
    fn status_snapshot() {
        let butler = ButlerService::new();
        butler.cache_hardware(GpuTier::CpuOnly);

        let status = butler.status();
        assert!(status.loaded_model.is_none());
        assert!(status.warm_endpoints.is_empty());
        assert_eq!(status.hardware_tier, Some(GpuTier::CpuOnly));
        assert!(status.active_operation.is_none());
    }

    #[test]
    fn mark_loaded_sets_model_without_guard() {
        let butler = ButlerService::new();
        butler.mark_loaded("medgemma:4b");
        assert!(butler.is_model_loaded("medgemma:4b"));
    }

    #[test]
    fn butler_status_serializes() {
        let status = ButlerStatus {
            loaded_model: Some("medgemma-4b".to_string()),
            warm_endpoints: vec![WarmEndpoint::Generate, WarmEndpoint::Chat],
            hardware_tier: Some(GpuTier::FullGpu),
            active_operation: None,
            idle_secs: 42,
            model_expires_at: Some("2026-02-27T03:00:00Z".to_string()),
            measured_tok_per_sec: Some(3.2),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("medgemma-4b"));
        assert!(json.contains("generate"));
        assert!(json.contains("chat"));
        assert!(json.contains("full_gpu"));
        assert!(json.contains("42"));
        assert!(json.contains("2026-02-27T03:00:00Z"));
        assert!(json.contains("3.2"));
    }

    // ── BTL-06: ensure_ready tests ──────────────────────────

    #[test]
    fn ensure_ready_skips_when_already_hot() {
        use crate::pipeline::structuring::ollama::OllamaClient;

        let butler = ButlerService::new();
        // Pre-set model as loaded + warm
        butler.mark_loaded("medgemma:4b");
        butler.mark_warm(WarmEndpoint::Generate);

        // ensure_ready should skip (unreachable Ollama would fail if it tried)
        let client = OllamaClient::new("http://localhost:99999");
        let result = butler.ensure_ready(&client, "medgemma:4b", WarmEndpoint::Generate);
        assert!(result.is_ok());
    }

    #[test]
    fn ensure_ready_attempts_warm_when_cold() {
        use crate::pipeline::structuring::ollama::OllamaClient;

        let butler = ButlerService::new();
        // Model not loaded — ensure_ready should try to warm (and fail on unreachable)
        let client = OllamaClient::new("http://localhost:99999");
        let result = butler.ensure_ready(&client, "medgemma:4b", WarmEndpoint::Generate);
        assert!(result.is_err()); // Can't reach Ollama
    }

    #[test]
    fn ensure_ready_attempts_chat_warm_when_only_generate_is_hot() {
        use crate::pipeline::structuring::ollama::OllamaClient;

        let butler = ButlerService::new();
        butler.mark_loaded("medgemma:4b");
        butler.mark_warm(WarmEndpoint::Generate);

        // Generate is hot but Chat is not — should attempt chat warm
        let client = OllamaClient::new("http://localhost:99999");
        let result = butler.ensure_ready(&client, "medgemma:4b", WarmEndpoint::Chat);
        assert!(result.is_err()); // Can't reach Ollama for chat warm
    }

    #[test]
    fn warm_endpoint_serializes() {
        let json = serde_json::to_string(&WarmEndpoint::Generate).unwrap();
        assert_eq!(json, "\"generate\"");
        let json = serde_json::to_string(&WarmEndpoint::Chat).unwrap();
        assert_eq!(json, "\"chat\"");
    }

    #[test]
    fn warm_state_serializes() {
        let json = serde_json::to_string(&WarmState::Hot).unwrap();
        assert_eq!(json, "\"hot\"");
        let json = serde_json::to_string(&WarmState::Cold).unwrap();
        assert_eq!(json, "\"cold\"");
        let json = serde_json::to_string(&WarmState::Loaded).unwrap();
        assert_eq!(json, "\"loaded\"");
    }

    // ── OLM-C3: Inference Metrics → Butler Integration ──

    #[test]
    fn record_metrics_stores_last_inference() {
        let butler = ButlerService::new();
        assert!(butler.measured_tokens_per_second().is_none());

        let metrics = InferenceMetrics {
            done_reason: None,
            total_duration_ns: Some(5_000_000_000),
            load_duration_ns: Some(100_000_000),
            prompt_eval_count: Some(10),
            prompt_eval_duration_ns: Some(500_000_000),
            eval_count: Some(100),
            eval_duration_ns: Some(2_000_000_000),
        };
        butler.record_metrics(Some(metrics));

        let tps = butler.measured_tokens_per_second().unwrap();
        assert!((tps - 50.0).abs() < 0.01, "Expected 50 tok/s, got {tps}");
    }

    #[test]
    fn record_metrics_none_is_noop() {
        let butler = ButlerService::new();
        butler.record_metrics(None);
        assert!(butler.measured_tokens_per_second().is_none());
    }

    #[test]
    fn record_metrics_updates_on_subsequent_calls() {
        let butler = ButlerService::new();

        let metrics1 = InferenceMetrics {
            done_reason: None,
            total_duration_ns: None,
            load_duration_ns: None,
            prompt_eval_count: None,
            prompt_eval_duration_ns: None,
            eval_count: Some(100),
            eval_duration_ns: Some(2_000_000_000),
        };
        butler.record_metrics(Some(metrics1));
        let tps1 = butler.measured_tokens_per_second().unwrap();

        let metrics2 = InferenceMetrics {
            done_reason: None,
            total_duration_ns: None,
            load_duration_ns: None,
            prompt_eval_count: None,
            prompt_eval_duration_ns: None,
            eval_count: Some(50),
            eval_duration_ns: Some(1_000_000_000),
        };
        butler.record_metrics(Some(metrics2));
        let tps2 = butler.measured_tokens_per_second().unwrap();

        assert!((tps1 - 50.0).abs() < 0.01);
        assert!((tps2 - 50.0).abs() < 0.01);
    }

    #[test]
    fn record_model_expires_at_surfaces_in_status() {
        let butler = ButlerService::new();
        butler.record_model_expires_at(Some("2026-02-27T03:00:00Z".to_string()));

        let status = butler.status();
        assert_eq!(status.model_expires_at.as_deref(), Some("2026-02-27T03:00:00Z"));
    }

    #[test]
    fn status_includes_measured_tok_per_sec() {
        let butler = ButlerService::new();
        let metrics = InferenceMetrics {
            done_reason: None,
            total_duration_ns: None,
            load_duration_ns: None,
            prompt_eval_count: None,
            prompt_eval_duration_ns: None,
            eval_count: Some(32),
            eval_duration_ns: Some(10_000_000_000),
        };
        butler.record_metrics(Some(metrics));

        let status = butler.status();
        let tps = status.measured_tok_per_sec.unwrap();
        assert!((tps - 3.2).abs() < 0.01, "Expected 3.2 tok/s, got {tps}");
    }

    #[test]
    fn status_defaults_without_metrics() {
        let butler = ButlerService::new();
        let status = butler.status();
        assert!(status.model_expires_at.is_none());
        assert!(status.measured_tok_per_sec.is_none());
    }

    // ── L6-11: ButlerSession tests ─────────────────────────

    #[test]
    fn start_session_resolves_strategy() {
        let butler = ButlerService::new();
        let session = butler
            .start_session(
                OperationKind::DocumentStructuring,
                "coheara-medgemma-4b-q8",
                ContextType::DocumentExtraction,
            )
            .unwrap();
        assert_eq!(session.context(), ContextType::DocumentExtraction);
        assert_eq!(session.model(), "coheara-medgemma-4b-q8");
        assert!(session.strategy().streaming);
        assert_eq!(session.strategy().max_tokens, 4096);
    }

    #[test]
    fn session_guard_config_derives_from_strategy() {
        let butler = ButlerService::new();
        let session = butler
            .start_session(
                OperationKind::BatchExtraction,
                "coheara-medgemma-4b-q4",
                ContextType::NightBatch,
            )
            .unwrap();
        // NightBatch uses IterativeDrill with 1024 max_tokens
        assert_eq!(session.strategy().max_tokens, 1024);
        assert_eq!(session.stream_guard_config().max_total_tokens, 1024);
        // Other guard defaults preserved
        assert_eq!(session.stream_guard_config().max_consecutive_identical, 20);
        assert_eq!(session.stream_guard_config().sequence_length, 10);
    }

    #[test]
    fn session_with_contract() {
        use crate::pipeline::domain_contracts;
        let butler = ButlerService::new();
        let session = butler
            .start_session(
                OperationKind::DocumentStructuring,
                "medgemma:4b",
                ContextType::DocumentExtraction,
            )
            .unwrap()
            .with_contract(&domain_contracts::LAB_RESULTS);
        assert!(session.contract().is_some());
        assert_eq!(session.contract().unwrap().domain, "lab_results");
    }

    #[test]
    fn session_without_contract() {
        let butler = ButlerService::new();
        let session = butler
            .start_session(
                OperationKind::ChatGeneration,
                "medgemma:4b",
                ContextType::HealthQuery,
            )
            .unwrap();
        assert!(session.contract().is_none());
    }

    #[test]
    fn session_sanitize_strips_thinking_tokens() {
        let butler = ButlerService::new();
        let session = butler
            .start_session(
                OperationKind::ChatGeneration,
                "medgemma:4b",
                ContextType::HealthQuery,
            )
            .unwrap();
        let raw = "<unused94>thought\nsome reasoning here\nActual answer text";
        let sanitized = session.sanitize(raw);
        assert!(sanitized.contains("Actual answer text"));
        assert!(!sanitized.contains("thought"));
    }

    #[test]
    fn session_validate_output_passes_healthy_text() {
        let butler = ButlerService::new();
        let session = butler
            .start_session(
                OperationKind::DocumentStructuring,
                "medgemma:4b",
                ContextType::DocumentExtraction,
            )
            .unwrap();
        let text = "The patient presented with persistent headaches \
                    over the past two weeks. Blood pressure was measured \
                    at 140/90 mmHg. Lab results show elevated creatinine.";
        assert!(session.validate_output(text).is_ok());
    }

    #[test]
    fn session_validate_output_catches_degenerate() {
        let butler = ButlerService::new();
        let session = butler
            .start_session(
                OperationKind::DocumentStructuring,
                "medgemma:4b",
                ContextType::DocumentExtraction,
            )
            .unwrap();
        let degenerate = vec!["error"; 100].join(" ");
        assert!(session.validate_output(&degenerate).is_err());
    }

    #[test]
    fn session_process_output_sanitizes_and_validates() {
        let butler = ButlerService::new();
        let session = butler
            .start_session(
                OperationKind::ChatGeneration,
                "medgemma:4b",
                ContextType::HealthQuery,
            )
            .unwrap();
        let raw = "<unused94>thought\nreasoning\n\
                   The patient has normal blood pressure at 120/80 mmHg with \
                   adequate hemoglobin levels and no signs of infection or \
                   abnormal lab findings during the routine screening.";
        let result = session.process_output(raw);
        assert!(result.is_ok());
        let clean = result.unwrap();
        assert!(!clean.contains("thought"));
        assert!(clean.contains("blood pressure"));
    }

    #[test]
    fn session_process_output_rejects_degenerate() {
        let butler = ButlerService::new();
        let session = butler
            .start_session(
                OperationKind::DocumentStructuring,
                "medgemma:4b",
                ContextType::DocumentExtraction,
            )
            .unwrap();
        let degenerate = vec!["error error error"; 50].join(" ");
        let result = session.process_output(&degenerate);
        assert!(result.is_err());
    }

    #[test]
    fn session_with_custom_quality_config() {
        let butler = ButlerService::new();
        let config = QualityGateConfig {
            min_diversity_ratio: 0.5,
            max_line_dominance: 0.3,
            min_words_for_check: 10,
        };
        let session = butler
            .start_session(
                OperationKind::DocumentStructuring,
                "medgemma:4b",
                ContextType::DocumentExtraction,
            )
            .unwrap()
            .with_quality_config(config);
        // Strict config catches more
        let text = "take one tablet daily with food take one tablet daily \
                    with food take one tablet daily with food and water";
        assert!(session.validate_output(text).is_err());
    }

    #[test]
    fn session_holds_exclusive_access() {
        let butler = ButlerService::new();
        let _session = butler
            .start_session(
                OperationKind::DocumentOcr,
                "medgemma:4b",
                ContextType::VisionOcr,
            )
            .unwrap();
        // Butler is busy — try_acquire should fail
        assert!(butler.try_acquire(OperationKind::ChatGeneration, "medgemma:4b").is_none());
    }

    #[test]
    fn session_drop_releases_access() {
        let butler = ButlerService::new();
        {
            let _session = butler
                .start_session(
                    OperationKind::DocumentOcr,
                    "medgemma:4b",
                    ContextType::VisionOcr,
                )
                .unwrap();
        }
        // After session drop, butler is free
        let guard = butler.try_acquire(OperationKind::ChatGeneration, "medgemma:4b");
        assert!(guard.is_some());
    }

    #[test]
    fn session_gpu_aware_strategy() {
        let butler = ButlerService::new();
        butler.cache_hardware(GpuTier::FullGpu);
        let session = butler
            .start_session(
                OperationKind::DocumentStructuring,
                "coheara-medgemma-4b-f16",
                ContextType::DocumentExtraction,
            )
            .unwrap();
        // GPU + F16 should still use MarkdownList (STR-01: context determines strategy)
        assert!(session.strategy().streaming);
    }

    #[test]
    fn derive_guard_config_caps_tokens() {
        use crate::pipeline::prompt_templates::PromptStrategyKind;
        let strategy = PromptStrategy {
            kind: PromptStrategyKind::IterativeDrill,
            temperature: 0.1,
            max_tokens: 1024,
            max_retries: 1,
            streaming: true,
        };
        let config = derive_guard_config(&strategy);
        assert_eq!(config.max_total_tokens, 1024);
        // Defaults preserved
        assert_eq!(config.max_consecutive_identical, 20);
        assert_eq!(config.sequence_length, 10);
        assert_eq!(config.max_sequence_repeats, 5);
        assert_eq!(config.ring_buffer_size, 200);
    }

    // ── C4: ButlerSession LLM method tests ─────────────────

    enum MockLlmMode {
        Ok(String),
        Degenerate,
    }

    /// Mock LLM client for testing session generate().
    struct MockLlm {
        mode: MockLlmMode,
    }

    impl MockLlm {
        fn ok(text: &str) -> Self {
            Self { mode: MockLlmMode::Ok(text.to_string()) }
        }

        fn degenerate() -> Self {
            Self { mode: MockLlmMode::Degenerate }
        }
    }

    impl crate::pipeline::structuring::types::LlmClient for MockLlm {
        fn generate(
            &self,
            _model: &str,
            _prompt: &str,
            _system: &str,
        ) -> Result<String, crate::pipeline::structuring::StructuringError> {
            match &self.mode {
                MockLlmMode::Ok(text) => Ok(text.clone()),
                MockLlmMode::Degenerate => Err(crate::pipeline::structuring::StructuringError::Degeneration {
                    pattern: "sequence_repeat".into(),
                    tokens_before_abort: 200,
                    partial_output: "repeat repeat".into(),
                }),
            }
        }

        fn is_model_available(&self, _model: &str) -> Result<bool, crate::pipeline::structuring::StructuringError> {
            Ok(true)
        }

        fn list_models(&self) -> Result<Vec<String>, crate::pipeline::structuring::StructuringError> {
            Ok(vec!["mock:latest".into()])
        }
    }

    enum MockVisionMode {
        Ok(String),
        Degenerate,
        NetworkError,
    }

    /// Mock vision client for testing session chat_with_images().
    struct MockVision {
        mode: MockVisionMode,
    }

    impl MockVision {
        fn ok(text: &str) -> Self {
            Self { mode: MockVisionMode::Ok(text.to_string()) }
        }

        fn degenerate() -> Self {
            Self { mode: MockVisionMode::Degenerate }
        }

        fn network_error() -> Self {
            Self { mode: MockVisionMode::NetworkError }
        }
    }

    impl crate::pipeline::structuring::types::VisionClient for MockVision {
        fn generate_with_images(
            &self,
            _model: &str,
            _prompt: &str,
            _images: &[String],
            _system: Option<&str>,
        ) -> Result<String, crate::pipeline::structuring::ollama_types::OllamaError> {
            match &self.mode {
                MockVisionMode::Ok(text) => Ok(text.clone()),
                MockVisionMode::Degenerate => Err(crate::pipeline::structuring::ollama_types::OllamaError::VisionDegeneration {
                    pattern: "token_repeat".into(),
                    tokens_before_abort: 150,
                    partial_output: "13.3 13.3 13.3".into(),
                }),
                MockVisionMode::NetworkError => Err(crate::pipeline::structuring::ollama_types::OllamaError::NotReachable),
            }
        }

        fn chat_with_images(
            &self,
            _model: &str,
            _user_prompt: &str,
            _images: &[String],
            _system: Option<&str>,
        ) -> Result<String, crate::pipeline::structuring::ollama_types::OllamaError> {
            match &self.mode {
                MockVisionMode::Ok(text) => Ok(text.clone()),
                MockVisionMode::Degenerate => Err(crate::pipeline::structuring::ollama_types::OllamaError::VisionDegeneration {
                    pattern: "token_repeat".into(),
                    tokens_before_abort: 150,
                    partial_output: "13.3 13.3 13.3".into(),
                }),
                MockVisionMode::NetworkError => Err(crate::pipeline::structuring::ollama_types::OllamaError::NotReachable),
            }
        }
    }

    #[test]
    fn session_generate_returns_validated_output() {
        let butler = ButlerService::new();
        let session = butler
            .start_session(OperationKind::DocumentStructuring, "medgemma:4b", ContextType::DocumentExtraction)
            .unwrap();
        let client = MockLlm::ok("Patient has normal blood pressure at 120/80 mmHg with adequate hemoglobin levels");
        let result = session.generate(&client, "extract data", "You are a medical assistant");
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.text.contains("blood pressure"));
        assert_eq!(output.model, "medgemma:4b");
        assert!(output.tokens_generated > 0);
    }

    #[test]
    fn session_generate_sanitizes_thinking_tokens() {
        let butler = ButlerService::new();
        let session = butler
            .start_session(OperationKind::ChatGeneration, "medgemma:4b", ContextType::HealthQuery)
            .unwrap();
        // sanitize_llm_output strips "<unusedN>thought\n" prefix and everything before it
        let client = MockLlm::ok(
            "<unused94>thought\nsome internal reasoning here\nThe patient has stable vital signs and normal lab results throughout the screening"
        );
        let result = session.generate(&client, "query", "system");
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.text.contains("<unused94>"));
        assert!(!output.text.contains("thought"));
        assert!(output.text.contains("vital signs"));
    }

    #[test]
    fn session_generate_maps_degeneration() {
        let butler = ButlerService::new();
        let session = butler
            .start_session(OperationKind::DocumentStructuring, "medgemma:4b", ContextType::DocumentExtraction)
            .unwrap();
        let client = MockLlm::degenerate();
        let result = session.generate(&client, "extract", "system");
        assert!(result.is_err());
        match result.unwrap_err() {
            SessionError::Degeneration { pattern, tokens_before_abort, .. } => {
                assert_eq!(pattern, "sequence_repeat");
                assert_eq!(tokens_before_abort, 200);
            }
            other => panic!("Expected Degeneration, got: {other}"),
        }
    }

    #[test]
    fn session_generate_rejects_degenerate_output() {
        let butler = ButlerService::new();
        let session = butler
            .start_session(OperationKind::DocumentStructuring, "medgemma:4b", ContextType::DocumentExtraction)
            .unwrap();
        let degenerate = vec!["Pharmacist Dr. LEVANDIER"; 100].join("\n");
        let client = MockLlm::ok(&degenerate);
        let result = session.generate(&client, "extract", "system");
        assert!(result.is_err());
        match result.unwrap_err() {
            SessionError::QualityGate { reason, raw_output } => {
                assert!(!reason.is_empty());
                assert!(raw_output.contains("LEVANDIER"));
            }
            other => panic!("Expected QualityGate, got: {other}"),
        }
    }

    #[test]
    fn session_chat_with_images_returns_validated_output() {
        let butler = ButlerService::new();
        let session = butler
            .start_session(OperationKind::DocumentOcr, "medgemma:4b", ContextType::VisionOcr)
            .unwrap();
        let client = MockVision::ok("Hemoglobin level measured at 13.3 g/dl in the laboratory report");
        let result = session.chat_with_images(
            &client, "What is the hemoglobin value?", &["base64img".into()], Some("system"),
        );
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.text.contains("13.3"));
        assert_eq!(output.model, "medgemma:4b");
    }

    #[test]
    fn session_chat_with_images_maps_degeneration() {
        let butler = ButlerService::new();
        let session = butler
            .start_session(OperationKind::DocumentOcr, "medgemma:4b", ContextType::VisionOcr)
            .unwrap();
        let client = MockVision::degenerate();
        let result = session.chat_with_images(&client, "extract", &["img".into()], None);
        assert!(result.is_err());
        match result.unwrap_err() {
            SessionError::Degeneration { pattern, tokens_before_abort, partial_output } => {
                assert_eq!(pattern, "token_repeat");
                assert_eq!(tokens_before_abort, 150);
                assert!(partial_output.contains("13.3"));
            }
            other => panic!("Expected Degeneration, got: {other}"),
        }
    }

    #[test]
    fn session_chat_with_images_maps_network_error() {
        let butler = ButlerService::new();
        let session = butler
            .start_session(OperationKind::DocumentOcr, "medgemma:4b", ContextType::VisionOcr)
            .unwrap();
        let client = MockVision::network_error();
        let result = session.chat_with_images(&client, "extract", &["img".into()], None);
        assert!(result.is_err());
        match result.unwrap_err() {
            SessionError::Llm(msg) => assert!(msg.contains("not running")),
            other => panic!("Expected Llm, got: {other}"),
        }
    }

    #[test]
    fn session_chat_with_images_rejects_degenerate_output() {
        let butler = ButlerService::new();
        let session = butler
            .start_session(OperationKind::DocumentOcr, "medgemma:4b", ContextType::VisionOcr)
            .unwrap();
        let degenerate = vec!["Lab Pharmacist: Dr. LEVANDIER"; 100].join("\n");
        let client = MockVision::ok(&degenerate);
        let result = session.chat_with_images(&client, "extract text", &["img".into()], None);
        assert!(result.is_err());
        matches!(result.unwrap_err(), SessionError::QualityGate { .. });
    }

    #[test]
    fn validated_output_fields() {
        let output = ValidatedOutput {
            text: "hello world test".to_string(),
            tokens_generated: 3,
            model: "test-model".to_string(),
        };
        assert_eq!(output.text, "hello world test");
        assert_eq!(output.tokens_generated, 3);
        assert_eq!(output.model, "test-model");
    }

    #[test]
    fn session_error_display() {
        let err = SessionError::Degeneration {
            pattern: "token_repeat".into(),
            tokens_before_abort: 100,
            partial_output: "partial".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("token_repeat"));
        assert!(msg.contains("100"));

        let err = SessionError::QualityGate {
            reason: "low diversity".into(),
            raw_output: "raw".into(),
        };
        assert!(err.to_string().contains("low diversity"));

        let err = SessionError::Llm("connection refused".into());
        assert!(err.to_string().contains("connection refused"));

        let inner = SessionError::Llm("inner".into());
        let err = SessionError::RetriesExhausted {
            attempts: 3,
            last_error: Box::new(inner),
        };
        assert!(err.to_string().contains("3 retries"));
    }

    #[test]
    fn session_error_from_ollama_service_error() {
        let svc_err = OllamaServiceError::LockPoisoned;
        let session_err: SessionError = svc_err.into();
        assert!(matches!(session_err, SessionError::Llm(_)));
    }

    /// Verify ButlerSession passes strategy params via chat_with_images_with_params.
    #[test]
    fn session_chat_uses_strategy_params() {
        use std::sync::atomic::{AtomicBool, Ordering};

        struct ParamsCapturingVision {
            params_used: AtomicBool,
        }

        impl crate::pipeline::structuring::types::VisionClient for ParamsCapturingVision {
            fn generate_with_images(
                &self, _: &str, _: &str, _: &[String], _: Option<&str>,
            ) -> Result<String, crate::pipeline::structuring::ollama_types::OllamaError> {
                Ok(String::new())
            }

            fn chat_with_images(
                &self, _: &str, _: &str, _: &[String], _: Option<&str>,
            ) -> Result<String, crate::pipeline::structuring::ollama_types::OllamaError> {
                // Should NOT be called if params method is used
                panic!("Expected chat_with_images_with_params, not chat_with_images");
            }

            fn chat_with_images_with_params(
                &self, _: &str, _: &str, _: &[String], _: Option<&str>,
                _params: crate::pipeline::structuring::types::VisionCallParams,
            ) -> Result<String, crate::pipeline::structuring::ollama_types::OllamaError> {
                self.params_used.store(true, Ordering::Relaxed);
                Ok("Patient has stable vital signs with blood pressure at normal range".into())
            }
        }

        let butler = ButlerService::new();
        let session = butler
            .start_session(OperationKind::DocumentOcr, "medgemma:4b", ContextType::VisionOcr)
            .unwrap();
        let client = ParamsCapturingVision {
            params_used: AtomicBool::new(false),
        };
        let result = session.chat_with_images(&client, "extract", &["img".into()], None);
        assert!(result.is_ok());
        assert!(client.params_used.load(Ordering::Relaxed));
    }

    // ── FallbackSession tests ────────────────────────────────

    #[test]
    fn fallback_session_model_returns_name() {
        let session = FallbackSession::new("medgemma:4b", ContextType::VisionOcr, false);
        assert_eq!(session.model(), "medgemma:4b");
    }

    #[test]
    fn fallback_session_implements_vision_session() {
        let session = FallbackSession::new("medgemma:4b", ContextType::NightBatch, false);

        // Verify it's usable as &dyn VisionSession
        let dyn_ref: &dyn VisionSession = &session;
        assert_eq!(dyn_ref.model(), "medgemma:4b");
    }

    #[test]
    fn fallback_session_chat_returns_validated_output() {
        struct EchoVision;
        impl VisionClient for EchoVision {
            fn generate_with_images(&self, _: &str, _: &str, _: &[String], _: Option<&str>) -> Result<String, OllamaError> {
                Ok(String::new())
            }
            fn chat_with_images(&self, _: &str, prompt: &str, _: &[String], _: Option<&str>) -> Result<String, OllamaError> {
                Ok(format!("response to: {prompt}"))
            }
        }

        let session = FallbackSession::new("medgemma:4b", ContextType::VisionOcr, false);
        let result = session.chat_with_images(&EchoVision, "test prompt", &["img".into()], None);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.text.contains("response to: test prompt"));
        assert_eq!(output.model, "medgemma:4b");
    }

    #[test]
    fn fallback_session_maps_degeneration_error() {
        struct DegenerateVision;
        impl VisionClient for DegenerateVision {
            fn generate_with_images(&self, _: &str, _: &str, _: &[String], _: Option<&str>) -> Result<String, OllamaError> {
                Ok(String::new())
            }
            fn chat_with_images(&self, _: &str, _: &str, _: &[String], _: Option<&str>) -> Result<String, OllamaError> {
                Err(OllamaError::VisionDegeneration {
                    pattern: "test_pattern".into(),
                    tokens_before_abort: 42,
                    partial_output: "partial...".into(),
                })
            }
        }

        let session = FallbackSession::new("medgemma:4b", ContextType::VisionOcr, false);
        let result = session.chat_with_images(&DegenerateVision, "test", &["img".into()], None);
        assert!(matches!(result, Err(SessionError::Degeneration { pattern, .. }) if pattern == "test_pattern"));
    }

    #[test]
    fn fallback_session_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FallbackSession>();
    }
}
