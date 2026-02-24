//! L6-04: Model Preferences — classification, resolution, and validation.
//!
//! This module is the single authority for "which model does this profile use?"
//! It replaces the hardcoded `MEDGEMMA_MODELS` constant with a user-driven
//! preference system that flows through to all AI pipelines.
//!
//! Key types:
//! - `ModelQuality` — Medical vs General classification
//! - `PreferenceSource` — Who set the preference (User, Wizard, Fallback)
//! - `ResolvedModel` — The resolved model for AI operations
//! - `ActiveModelResolver` — Singleton resolver with 60s cache
//! - `PreferenceError` — Dedicated error enum

use std::str::FromStr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::db::DatabaseError;
use crate::pipeline::structuring::ollama_types::ModelRole;
use crate::pipeline::structuring::types::LlmClient;

// ── Enums ──────────────────────────────────────────────────────

/// How the model was classified (AD-07).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ModelQuality {
    Medical,
    General,
    Unknown,
}

impl std::fmt::Display for ModelQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Medical => write!(f, "medical"),
            Self::General => write!(f, "general"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

impl FromStr for ModelQuality {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "medical" => Ok(Self::Medical),
            "general" => Ok(Self::General),
            "unknown" => Ok(Self::Unknown),
            other => Err(format!("Invalid model quality: {other}")),
        }
    }
}

/// Who set the preference.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PreferenceSource {
    User,
    Wizard,
    Fallback,
}

impl std::fmt::Display for PreferenceSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Wizard => write!(f, "wizard"),
            Self::Fallback => write!(f, "fallback"),
        }
    }
}

impl FromStr for PreferenceSource {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user" => Ok(Self::User),
            "wizard" => Ok(Self::Wizard),
            "fallback" => Ok(Self::Fallback),
            other => Err(format!("Invalid preference source: {other}")),
        }
    }
}

// ── Data Structures ────────────────────────────────────────────

/// The resolved model for AI operations (AD-08).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedModel {
    pub name: String,
    pub quality: ModelQuality,
    pub source: PreferenceSource,
}

/// Stored model preference (maps to model_preferences table).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredModelPreference {
    pub active_model: Option<String>,
    pub model_quality: ModelQuality,
    pub set_at: Option<String>,
    pub set_by: PreferenceSource,
}

// ── Constants ──────────────────────────────────────────────────

/// Prefixes that identify medical-domain models (AD-07).
///
/// Maintained as a curated list — not inferred from model metadata.
/// The Ollama registry has no "medical" tag, so we maintain our own.
const MEDICAL_MODEL_PREFIXES: &[&str] = &[
    "medgemma",
    "biomistral",
    "meditron",
    "med-",
    "medical",
    "biomedical",
    "clinical",
    "pubmed",
];

/// Whitelisted preference keys for user_preferences table (SEC-L6-16).
///
/// Only these keys can be written. Prevents the table from becoming
/// an arbitrary data store.
const ALLOWED_PREFERENCE_KEYS: &[&str] = &[
    "dismissed_ai_setup",
    "theme",
    "language",
    "sidebar_collapsed",
];

// ── Classification (pure) ──────────────────────────────────────

/// Classify a model name as Medical or General based on curated prefix list.
///
/// DOM-L6-13: Classification is informational only — does NOT gate features.
///
/// R-MOD-02 F1: Uses `extract_model_component()` to strip namespace prefix
/// before matching. `dcarrascosa/medgemma-1.5-4b-it` → extracts `medgemma-1.5-4b-it` →
/// matches `medgemma` prefix → `Medical`.
pub fn classify_model(model_name: &str) -> ModelQuality {
    let component = super::ollama_types::extract_model_component(model_name);
    for prefix in MEDICAL_MODEL_PREFIXES {
        if component.starts_with(prefix) {
            return ModelQuality::Medical;
        }
    }
    ModelQuality::General
}

/// Validate a preference key against the whitelist (SEC-L6-16).
pub fn validate_preference_key(key: &str) -> Result<(), PreferenceError> {
    if ALLOWED_PREFERENCE_KEYS.contains(&key) {
        Ok(())
    } else {
        Err(PreferenceError::InvalidPreferenceKey(key.to_string()))
    }
}

// ── Error Type ─────────────────────────────────────────────────

/// Errors from model preference operations.
#[derive(Debug, thiserror::Error)]
pub enum PreferenceError {
    #[error("Invalid model name: {0}")]
    InvalidModelName(String),

    #[error("Invalid preference key: {0}. Allowed keys: dismissed_ai_setup, theme, language, sidebar_collapsed.")]
    InvalidPreferenceKey(String),

    #[error("No AI model is available. Please install a model using Ollama.")]
    NoModelAvailable,

    #[error("Ollama is not reachable: {0}")]
    OllamaUnavailable(String),

    /// No vision-capable model available for OCR operations.
    #[error("No vision-capable model is available. Install a vision model (e.g., MedGemma) for document extraction.")]
    NoVisionModelAvailable,

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),
}

// ── ActiveModelResolver ────────────────────────────────────────

/// Resolves the active model for AI operations (AD-06).
///
/// Caches installed model list for 60s to avoid repeated Ollama HTTP calls.
/// Singleton managed by CoreState (Q-14) — all pipelines share the cache.
///
/// Resolution order:
/// 1. User preference (if set and still installed)
/// 2. First medical model from installed list
/// 3. First any model from installed list
/// 4. Error: no model available
pub struct ActiveModelResolver {
    /// Cached installed models list with timestamp.
    cache: Mutex<Option<(Vec<String>, Instant)>>,
    /// Cache TTL (default 60s).
    cache_ttl: Duration,
}

impl ActiveModelResolver {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(None),
            cache_ttl: Duration::from_secs(60),
        }
    }

    /// Resolve the active model for this profile.
    ///
    /// Takes a DB connection (for reading stored preference) and an LlmClient
    /// (for verifying which models are installed). Uses trait-based DI for
    /// testability (QA-L6-16).
    pub fn resolve(
        &self,
        conn: &rusqlite::Connection,
        client: &dyn LlmClient,
    ) -> Result<ResolvedModel, PreferenceError> {
        let installed = self.get_installed_models(client)?;
        let pref = crate::db::repository::get_model_preference(conn)?;

        // Step 1: User preference — verify still installed
        if let Some(ref model_name) = pref.active_model {
            let is_installed = installed.iter().any(|m| {
                m == model_name || m.starts_with(&format!("{model_name}:"))
            });
            if is_installed {
                return Ok(ResolvedModel {
                    name: model_name.clone(),
                    quality: pref.model_quality,
                    source: pref.set_by,
                });
            }
            // Preference is stale — model was uninstalled (T-14)
            tracing::warn!(
                model = model_name,
                "Preferred model no longer installed, falling back"
            );
        }

        // Step 2: First medical model (DOM-L6-14)
        for model in &installed {
            if classify_model(model) == ModelQuality::Medical {
                return Ok(ResolvedModel {
                    name: model.clone(),
                    quality: ModelQuality::Medical,
                    source: PreferenceSource::Fallback,
                });
            }
        }

        // Step 3: Any model
        if let Some(model) = installed.first() {
            return Ok(ResolvedModel {
                name: model.clone(),
                quality: classify_model(model),
                source: PreferenceSource::Fallback,
            });
        }

        // Step 4: Nothing available
        Err(PreferenceError::NoModelAvailable)
    }

    /// Resolve the active model for a specific pipeline role.
    ///
    /// Role-based resolution enables different models for different tasks:
    /// - `LlmGeneration` → standard resolve() chain (text generation, structuring)
    /// - `VisionOcr` → vision-specific chain (OCR preference → vision-capable fallback)
    ///
    /// Currently MedGemma serves both roles, but this architecture supports
    /// adding specialized models (e.g., a dedicated OCR model) without changing callers.
    pub fn resolve_for_role(
        &self,
        role: ModelRole,
        conn: &rusqlite::Connection,
        client: &dyn LlmClient,
    ) -> Result<ResolvedModel, PreferenceError> {
        match role {
            ModelRole::LlmGeneration => self.resolve(conn, client),
            ModelRole::VisionOcr => self.resolve_vision_ocr(conn, client),
        }
    }

    /// Vision OCR resolution chain.
    ///
    /// Resolution order (modular — supports future specialist models):
    /// 1. User-set OCR model preference (`active_ocr_model`) if installed
    /// 2. Any vision-capable installed model (MedGemma, LLaVA, etc.)
    /// 3. Error: no vision model available
    fn resolve_vision_ocr(
        &self,
        conn: &rusqlite::Connection,
        client: &dyn LlmClient,
    ) -> Result<ResolvedModel, PreferenceError> {
        let installed = self.get_installed_models(client)?;

        // Step 1: Explicit OCR model preference (user choice takes priority)
        let ocr_pref = crate::db::repository::get_ocr_model_preference(conn)?;
        if let Some(ref model_name) = ocr_pref {
            let is_installed = installed.iter().any(|m| {
                m == model_name || m.starts_with(&format!("{model_name}:"))
            });
            if is_installed {
                return Ok(ResolvedModel {
                    name: model_name.clone(),
                    quality: classify_model(model_name),
                    source: PreferenceSource::User,
                });
            }
            tracing::warn!(
                model = model_name,
                "OCR model preference not installed, falling back"
            );
        }

        // Step 2: Any vision-capable model
        for model in &installed {
            if super::ollama_types::is_vision_model(model) {
                return Ok(ResolvedModel {
                    name: model.clone(),
                    quality: classify_model(model),
                    source: PreferenceSource::Fallback,
                });
            }
        }

        // Step 3: No vision model available
        Err(PreferenceError::NoVisionModelAvailable)
    }

    /// Invalidate the cached installed models list.
    ///
    /// Called after pull_ollama_model, delete_ollama_model, set_active_model.
    pub fn invalidate_cache(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            *cache = None;
        }
    }

    /// Get installed models (from cache or LlmClient).
    fn get_installed_models(
        &self,
        client: &dyn LlmClient,
    ) -> Result<Vec<String>, PreferenceError> {
        if let Ok(mut cache) = self.cache.lock() {
            // Check cache validity
            if let Some((ref models, ref timestamp)) = *cache {
                if timestamp.elapsed() < self.cache_ttl {
                    return Ok(models.clone());
                }
            }

            // Cache miss or expired — refresh
            let models = client
                .list_models()
                .map_err(|e| PreferenceError::OllamaUnavailable(e.to_string()))?;
            *cache = Some((models.clone(), Instant::now()));
            Ok(models)
        } else {
            // Lock poisoned — fall through without cache
            client
                .list_models()
                .map_err(|e| PreferenceError::OllamaUnavailable(e.to_string()))
        }
    }
}

impl Default for ActiveModelResolver {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;
    use crate::pipeline::structuring::types::LlmClient;
    use crate::pipeline::structuring::StructuringError;

    // ── Mock LlmClient for resolver tests ───────────────────

    struct MockLlmForResolver {
        models: Vec<String>,
    }

    impl MockLlmForResolver {
        fn with_models(models: Vec<&str>) -> Self {
            Self {
                models: models.into_iter().map(String::from).collect(),
            }
        }

        fn empty() -> Self {
            Self { models: vec![] }
        }
    }

    impl LlmClient for MockLlmForResolver {
        fn generate(
            &self,
            _model: &str,
            _prompt: &str,
            _system_prompt: &str,
        ) -> Result<String, StructuringError> {
            Ok(String::new())
        }

        fn is_model_available(&self, model: &str) -> Result<bool, StructuringError> {
            Ok(self.models.iter().any(|m| m == model))
        }

        fn list_models(&self) -> Result<Vec<String>, StructuringError> {
            Ok(self.models.clone())
        }
    }

    // ── classify_model tests ────────────────────────────────

    mod classify_tests {
        use super::*;

        #[test]
        fn medgemma_is_medical() {
            assert_eq!(classify_model("medgemma:4b"), ModelQuality::Medical);
            assert_eq!(classify_model("medgemma:27b"), ModelQuality::Medical);
            assert_eq!(classify_model("medgemma:latest"), ModelQuality::Medical);
        }

        #[test]
        fn biomistral_is_medical() {
            assert_eq!(classify_model("biomistral:7b"), ModelQuality::Medical);
        }

        #[test]
        fn meditron_is_medical() {
            assert_eq!(classify_model("meditron:7b"), ModelQuality::Medical);
        }

        #[test]
        fn med_prefix_is_medical() {
            assert_eq!(classify_model("med-palm:latest"), ModelQuality::Medical);
        }

        #[test]
        fn medical_prefix_is_medical() {
            assert_eq!(classify_model("medical-llm:3b"), ModelQuality::Medical);
        }

        #[test]
        fn biomedical_is_medical() {
            assert_eq!(classify_model("biomedical-gpt:7b"), ModelQuality::Medical);
        }

        #[test]
        fn clinical_is_medical() {
            assert_eq!(classify_model("clinical-bert:latest"), ModelQuality::Medical);
        }

        #[test]
        fn pubmed_is_medical() {
            assert_eq!(classify_model("pubmed-llm:3b"), ModelQuality::Medical);
        }

        #[test]
        fn case_insensitive() {
            assert_eq!(classify_model("MedGemma:4b"), ModelQuality::Medical);
            assert_eq!(classify_model("MEDGEMMA:27B"), ModelQuality::Medical);
            assert_eq!(classify_model("BioMistral:7B"), ModelQuality::Medical);
        }

        #[test]
        fn general_models() {
            assert_eq!(classify_model("llama3:8b"), ModelQuality::General);
            assert_eq!(classify_model("mistral:7b"), ModelQuality::General);
            assert_eq!(classify_model("phi3:mini"), ModelQuality::General);
            assert_eq!(classify_model("gemma:7b"), ModelQuality::General);
        }

        #[test]
        fn empty_string_is_general() {
            assert_eq!(classify_model(""), ModelQuality::General);
        }

        // ── Namespaced model classification (R-MOD-02 L.3) ──

        #[test]
        fn namespaced_medgemma_is_medical() {
            assert_eq!(classify_model("dcarrascosa/medgemma-1.5-4b-it"), ModelQuality::Medical);
        }

        #[test]
        fn namespaced_medgemma_alt_is_medical() {
            assert_eq!(classify_model("amsaravi/medgemma-4b-it"), ModelQuality::Medical);
        }

        #[test]
        fn namespaced_biomistral_is_medical() {
            assert_eq!(classify_model("SomeOrg/biomistral:7b"), ModelQuality::Medical);
        }

        #[test]
        fn namespaced_med_prefix_is_medical() {
            assert_eq!(classify_model("org/med-palm:latest"), ModelQuality::Medical);
        }

        #[test]
        fn namespaced_clinical_is_medical() {
            assert_eq!(classify_model("user/clinical-bert:large"), ModelQuality::Medical);
        }

        #[test]
        fn namespaced_general_is_general() {
            assert_eq!(classify_model("random-org/general-model:latest"), ModelQuality::General);
        }

        #[test]
        fn medical_org_general_model_is_general() {
            // Org name contains "medical" but model itself doesn't — classify by model, not org
            assert_eq!(classify_model("medicalorg/llama:7b"), ModelQuality::General);
        }

        #[test]
        fn namespaced_llama_is_general() {
            assert_eq!(classify_model("meta/llama3:8b"), ModelQuality::General);
        }
    }

    // ── validate_preference_key tests ───────────────────────

    mod preference_key_tests {
        use super::*;

        #[test]
        fn allowed_keys_pass() {
            assert!(validate_preference_key("dismissed_ai_setup").is_ok());
            assert!(validate_preference_key("theme").is_ok());
            assert!(validate_preference_key("language").is_ok());
            assert!(validate_preference_key("sidebar_collapsed").is_ok());
        }

        #[test]
        fn disallowed_keys_rejected() {
            assert!(validate_preference_key("evil_key").is_err());
            assert!(validate_preference_key("").is_err());
            assert!(validate_preference_key("admin_mode").is_err());
            assert!(validate_preference_key("../../etc/passwd").is_err());
        }

        #[test]
        fn error_message_lists_allowed_keys() {
            let err = validate_preference_key("bad_key").unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains("dismissed_ai_setup"));
            assert!(msg.contains("bad_key"));
        }
    }

    // ── ModelQuality serialization tests ────────────────────

    mod quality_tests {
        use super::*;

        #[test]
        fn display_roundtrip() {
            assert_eq!(ModelQuality::Medical.to_string(), "medical");
            assert_eq!(ModelQuality::General.to_string(), "general");
            assert_eq!(ModelQuality::Unknown.to_string(), "unknown");

            assert_eq!("medical".parse::<ModelQuality>().unwrap(), ModelQuality::Medical);
            assert_eq!("general".parse::<ModelQuality>().unwrap(), ModelQuality::General);
            assert_eq!("unknown".parse::<ModelQuality>().unwrap(), ModelQuality::Unknown);
        }

        #[test]
        fn invalid_quality_errors() {
            assert!("invalid".parse::<ModelQuality>().is_err());
        }
    }

    // ── PreferenceSource serialization tests ────────────────

    mod source_tests {
        use super::*;

        #[test]
        fn display_roundtrip() {
            assert_eq!(PreferenceSource::User.to_string(), "user");
            assert_eq!(PreferenceSource::Wizard.to_string(), "wizard");
            assert_eq!(PreferenceSource::Fallback.to_string(), "fallback");

            assert_eq!("user".parse::<PreferenceSource>().unwrap(), PreferenceSource::User);
            assert_eq!("wizard".parse::<PreferenceSource>().unwrap(), PreferenceSource::Wizard);
            assert_eq!("fallback".parse::<PreferenceSource>().unwrap(), PreferenceSource::Fallback);
        }

        #[test]
        fn invalid_source_errors() {
            assert!("invalid".parse::<PreferenceSource>().is_err());
        }
    }

    // ── ResolvedModel tests ─────────────────────────────────

    #[test]
    fn resolved_model_serializes() {
        let model = ResolvedModel {
            name: "medgemma:4b".to_string(),
            quality: ModelQuality::Medical,
            source: PreferenceSource::User,
        };
        let json = serde_json::to_string(&model).unwrap();
        assert!(json.contains("\"name\":\"medgemma:4b\""));
        assert!(json.contains("\"quality\":\"Medical\""));
        assert!(json.contains("\"source\":\"User\""));
    }

    // ── PreferenceError tests ───────────────────────────────

    #[test]
    fn error_messages_are_complete_sentences() {
        let errors = vec![
            PreferenceError::InvalidModelName("bad".into()),
            PreferenceError::InvalidPreferenceKey("evil".into()),
            PreferenceError::NoModelAvailable,
            PreferenceError::OllamaUnavailable("timeout".into()),
        ];
        for err in errors {
            let msg = err.to_string();
            assert!(!msg.is_empty(), "Error message should not be empty");
            // Should be a complete, readable message
            assert!(
                msg.len() > 10,
                "Error message too short: {msg}"
            );
        }
    }

    // ── ActiveModelResolver tests ───────────────────────────

    mod resolve_tests {
        use super::*;

        fn setup_db_with_preference(
            model: Option<&str>,
            quality: &str,
            source: &str,
        ) -> rusqlite::Connection {
            let conn = open_memory_database().unwrap();
            if let Some(name) = model {
                conn.execute(
                    "UPDATE model_preferences SET active_model = ?1, model_quality = ?2, set_at = datetime('now'), set_by = ?3 WHERE id = 1",
                    rusqlite::params![name, quality, source],
                ).unwrap();
            }
            conn
        }

        #[test]
        fn user_preference_honored() {
            let conn = setup_db_with_preference(Some("medgemma:4b"), "medical", "user");
            let client = MockLlmForResolver::with_models(vec!["medgemma:4b", "llama3:8b"]);
            let resolver = ActiveModelResolver::new();

            let result = resolver.resolve(&conn, &client).unwrap();
            assert_eq!(result.name, "medgemma:4b");
            assert_eq!(result.quality, ModelQuality::Medical);
            assert_eq!(result.source, PreferenceSource::User);
        }

        #[test]
        fn stale_preference_falls_back_to_medical() {
            // User chose medgemma:4b but it was uninstalled. biomistral:7b is available.
            let conn = setup_db_with_preference(Some("medgemma:4b"), "medical", "user");
            let client = MockLlmForResolver::with_models(vec!["biomistral:7b", "llama3:8b"]);
            let resolver = ActiveModelResolver::new();

            let result = resolver.resolve(&conn, &client).unwrap();
            assert_eq!(result.name, "biomistral:7b");
            assert_eq!(result.quality, ModelQuality::Medical);
            assert_eq!(result.source, PreferenceSource::Fallback);
        }

        #[test]
        fn stale_preference_falls_back_to_any() {
            // User chose medgemma:4b but only general models available.
            let conn = setup_db_with_preference(Some("medgemma:4b"), "medical", "user");
            let client = MockLlmForResolver::with_models(vec!["llama3:8b"]);
            let resolver = ActiveModelResolver::new();

            let result = resolver.resolve(&conn, &client).unwrap();
            assert_eq!(result.name, "llama3:8b");
            assert_eq!(result.quality, ModelQuality::General);
            assert_eq!(result.source, PreferenceSource::Fallback);
        }

        #[test]
        fn no_models_returns_error() {
            let conn = setup_db_with_preference(None, "unknown", "user");
            let client = MockLlmForResolver::empty();
            let resolver = ActiveModelResolver::new();

            let result = resolver.resolve(&conn, &client);
            assert!(matches!(result, Err(PreferenceError::NoModelAvailable)));
        }

        #[test]
        fn no_preference_selects_best_medical() {
            let conn = setup_db_with_preference(None, "unknown", "user");
            let client = MockLlmForResolver::with_models(vec!["llama3:8b", "medgemma:4b"]);
            let resolver = ActiveModelResolver::new();

            let result = resolver.resolve(&conn, &client).unwrap();
            assert_eq!(result.name, "medgemma:4b");
            assert_eq!(result.quality, ModelQuality::Medical);
            assert_eq!(result.source, PreferenceSource::Fallback);
        }

        #[test]
        fn no_preference_no_medical_selects_first() {
            let conn = setup_db_with_preference(None, "unknown", "user");
            let client = MockLlmForResolver::with_models(vec!["llama3:8b", "mistral:7b"]);
            let resolver = ActiveModelResolver::new();

            let result = resolver.resolve(&conn, &client).unwrap();
            assert_eq!(result.name, "llama3:8b");
            assert_eq!(result.quality, ModelQuality::General);
            assert_eq!(result.source, PreferenceSource::Fallback);
        }

        #[test]
        fn wizard_source_preserved() {
            let conn = setup_db_with_preference(Some("medgemma:4b"), "medical", "wizard");
            let client = MockLlmForResolver::with_models(vec!["medgemma:4b"]);
            let resolver = ActiveModelResolver::new();

            let result = resolver.resolve(&conn, &client).unwrap();
            assert_eq!(result.source, PreferenceSource::Wizard);
        }

        #[test]
        fn resolver_selects_namespaced_medical_model() {
            // R-MOD-02: Resolver Step 2 must find namespaced medical model
            let conn = setup_db_with_preference(None, "unknown", "user");
            let client = MockLlmForResolver::with_models(vec![
                "llama3:8b",
                "dcarrascosa/medgemma-1.5-4b-it",
            ]);
            let resolver = ActiveModelResolver::new();

            let result = resolver.resolve(&conn, &client).unwrap();
            assert_eq!(result.name, "dcarrascosa/medgemma-1.5-4b-it");
            assert_eq!(result.quality, ModelQuality::Medical);
            assert_eq!(result.source, PreferenceSource::Fallback);
        }

        // ── Role-based resolution tests ──────────────

        #[test]
        fn llm_role_uses_standard_resolution() {
            let conn = setup_db_with_preference(Some("medgemma:4b"), "medical", "user");
            let client = MockLlmForResolver::with_models(vec!["medgemma:4b"]);
            let resolver = ActiveModelResolver::new();

            let result = resolver
                .resolve_for_role(ModelRole::LlmGeneration, &conn, &client)
                .unwrap();
            assert_eq!(result.name, "medgemma:4b");
        }

        #[test]
        fn vision_role_prefers_explicit_ocr_preference() {
            let conn = setup_db_with_preference(None, "unknown", "user");
            crate::db::repository::set_ocr_model_preference(&conn, "medgemma:4b")
                .unwrap();
            let client = MockLlmForResolver::with_models(vec![
                "llama3:8b",
                "medgemma:4b",
            ]);
            let resolver = ActiveModelResolver::new();

            let result = resolver
                .resolve_for_role(ModelRole::VisionOcr, &conn, &client)
                .unwrap();
            assert_eq!(result.name, "medgemma:4b");
            assert_eq!(result.source, PreferenceSource::User);
        }

        #[test]
        fn vision_role_falls_back_to_vision_model() {
            let conn = setup_db_with_preference(None, "unknown", "user");
            let client = MockLlmForResolver::with_models(vec![
                "llama3:8b",
                "dcarrascosa/medgemma-1.5-4b-it",
            ]);
            let resolver = ActiveModelResolver::new();

            let result = resolver
                .resolve_for_role(ModelRole::VisionOcr, &conn, &client)
                .unwrap();
            assert_eq!(result.name, "dcarrascosa/medgemma-1.5-4b-it");
            assert_eq!(result.source, PreferenceSource::Fallback);
        }

        #[test]
        fn vision_role_errors_when_no_vision_model() {
            let conn = setup_db_with_preference(None, "unknown", "user");
            let client = MockLlmForResolver::with_models(vec!["llama3:8b", "mistral:7b"]);
            let resolver = ActiveModelResolver::new();

            let result = resolver.resolve_for_role(ModelRole::VisionOcr, &conn, &client);
            assert!(matches!(result, Err(PreferenceError::NoVisionModelAvailable)));
        }

        #[test]
        fn vision_role_stale_pref_falls_back() {
            let conn = setup_db_with_preference(None, "unknown", "user");
            // Set OCR preference to a model that's NOT installed
            crate::db::repository::set_ocr_model_preference(&conn, "some-ocr-model:latest")
                .unwrap();
            let client = MockLlmForResolver::with_models(vec!["medgemma:4b"]);
            let resolver = ActiveModelResolver::new();

            let result = resolver
                .resolve_for_role(ModelRole::VisionOcr, &conn, &client)
                .unwrap();
            // Falls back to medgemma (vision-capable)
            assert_eq!(result.name, "medgemma:4b");
            assert_eq!(result.source, PreferenceSource::Fallback);
        }

        #[test]
        fn cache_invalidation_works() {
            let resolver = ActiveModelResolver::new();
            let client = MockLlmForResolver::with_models(vec!["medgemma:4b"]);

            // Prime the cache
            let _ = resolver.get_installed_models(&client);
            assert!(resolver.cache.lock().unwrap().is_some());

            // Invalidate
            resolver.invalidate_cache();
            assert!(resolver.cache.lock().unwrap().is_none());
        }
    }

    // ── Repository integration tests ────────────────────────

    mod persistence_tests {
        use super::*;
        use crate::db::repository;

        #[test]
        fn default_preference_is_null() {
            let conn = open_memory_database().unwrap();
            let pref = repository::get_model_preference(&conn).unwrap();
            assert!(pref.active_model.is_none());
            assert_eq!(pref.model_quality, ModelQuality::Unknown);
            assert_eq!(pref.set_by, PreferenceSource::User);
        }

        #[test]
        fn set_and_get_roundtrip() {
            let conn = open_memory_database().unwrap();
            repository::set_model_preference(
                &conn,
                "medgemma:4b",
                &ModelQuality::Medical,
                &PreferenceSource::User,
            )
            .unwrap();

            let pref = repository::get_model_preference(&conn).unwrap();
            assert_eq!(pref.active_model.as_deref(), Some("medgemma:4b"));
            assert_eq!(pref.model_quality, ModelQuality::Medical);
            assert_eq!(pref.set_by, PreferenceSource::User);
            assert!(pref.set_at.is_some());
        }

        #[test]
        fn clear_resets_to_null() {
            let conn = open_memory_database().unwrap();
            repository::set_model_preference(
                &conn,
                "medgemma:4b",
                &ModelQuality::Medical,
                &PreferenceSource::User,
            )
            .unwrap();
            repository::clear_model_preference(&conn).unwrap();

            let pref = repository::get_model_preference(&conn).unwrap();
            assert!(pref.active_model.is_none());
            assert_eq!(pref.model_quality, ModelQuality::Unknown);
        }

        #[test]
        fn set_classifies_quality() {
            let conn = open_memory_database().unwrap();
            // Set a general model
            repository::set_model_preference(
                &conn,
                "llama3:8b",
                &ModelQuality::General,
                &PreferenceSource::User,
            )
            .unwrap();

            let pref = repository::get_model_preference(&conn).unwrap();
            assert_eq!(pref.model_quality, ModelQuality::General);
        }

        #[test]
        fn wizard_source_persisted() {
            let conn = open_memory_database().unwrap();
            repository::set_model_preference(
                &conn,
                "medgemma:4b",
                &ModelQuality::Medical,
                &PreferenceSource::Wizard,
            )
            .unwrap();

            let pref = repository::get_model_preference(&conn).unwrap();
            assert_eq!(pref.set_by, PreferenceSource::Wizard);
        }
    }

    // ── OCR model preference persistence tests ─────────────

    mod ocr_preference_tests {
        use super::*;
        use crate::db::repository;

        #[test]
        fn default_ocr_preference_is_none() {
            let conn = open_memory_database().unwrap();
            let pref = repository::get_ocr_model_preference(&conn).unwrap();
            assert!(pref.is_none());
        }

        #[test]
        fn set_and_get_ocr_roundtrip() {
            let conn = open_memory_database().unwrap();
            repository::set_ocr_model_preference(&conn, "medgemma:4b").unwrap();

            let pref = repository::get_ocr_model_preference(&conn).unwrap();
            assert_eq!(pref.as_deref(), Some("medgemma:4b"));
        }

        #[test]
        fn clear_ocr_resets_to_none() {
            let conn = open_memory_database().unwrap();
            repository::set_ocr_model_preference(&conn, "medgemma:4b").unwrap();
            repository::clear_ocr_model_preference(&conn).unwrap();

            let pref = repository::get_ocr_model_preference(&conn).unwrap();
            assert!(pref.is_none());
        }

        #[test]
        fn ocr_preference_independent_of_llm_preference() {
            let conn = open_memory_database().unwrap();
            repository::set_model_preference(
                &conn,
                "dcarrascosa/medgemma-1.5-4b-it",
                &ModelQuality::Medical,
                &PreferenceSource::User,
            )
            .unwrap();
            repository::set_ocr_model_preference(&conn, "amsaravi/medgemma-4b-it").unwrap();

            // Both preferences exist independently
            let llm_pref = repository::get_model_preference(&conn).unwrap();
            let ocr_pref = repository::get_ocr_model_preference(&conn).unwrap();
            assert_eq!(llm_pref.active_model.as_deref(), Some("dcarrascosa/medgemma-1.5-4b-it"));
            assert_eq!(ocr_pref.as_deref(), Some("amsaravi/medgemma-4b-it"));
        }
    }

    // ── User preference tests ───────────────────────────────

    mod user_preference_tests {
        use super::*;
        use crate::db::repository;

        #[test]
        fn set_and_get_allowed_key() {
            let conn = open_memory_database().unwrap();
            repository::set_user_preference(&conn, "dismissed_ai_setup", "true").unwrap();

            let val = repository::get_user_preference(&conn, "dismissed_ai_setup").unwrap();
            assert_eq!(val.as_deref(), Some("true"));
        }

        #[test]
        fn get_nonexistent_returns_none() {
            let conn = open_memory_database().unwrap();
            let val = repository::get_user_preference(&conn, "theme").unwrap();
            assert!(val.is_none());
        }

        #[test]
        fn update_existing_key() {
            let conn = open_memory_database().unwrap();
            repository::set_user_preference(&conn, "theme", "light").unwrap();
            repository::set_user_preference(&conn, "theme", "dark").unwrap();

            let val = repository::get_user_preference(&conn, "theme").unwrap();
            assert_eq!(val.as_deref(), Some("dark"));
        }

        #[test]
        fn delete_preference() {
            let conn = open_memory_database().unwrap();
            repository::set_user_preference(&conn, "language", "fr").unwrap();
            repository::delete_user_preference(&conn, "language").unwrap();

            let val = repository::get_user_preference(&conn, "language").unwrap();
            assert!(val.is_none());
        }
    }
}
