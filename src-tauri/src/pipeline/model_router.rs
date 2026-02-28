//! CT-01: Model Router — tag-driven pipeline assignment.
//!
//! Resolves which model handles each pipeline stage based on:
//! 1. User-assigned capability tags (VISION, MEDICAL, TXT, etc.)
//! 2. Enabled/disabled flag per model
//! 3. Prefix heuristic fallback (backward compatible)
//!
//! Produces a `PipelineAssignment` that tells the processor:
//! - Which extraction strategy to use (VisionOcr, PdfiumText, DirectText)
//! - Which model handles structuring
//! - Which processing mode to use (Interleaved vs BatchStages)

use std::collections::HashMap;

use rusqlite::Connection;

use crate::db::repository;
use crate::pipeline::import::format::FileCategory;
use crate::pipeline::structuring::ollama_types::{normalize_model_identity, CapabilityTag};
use crate::pipeline::structuring::preferences::{ActiveModelResolver, PreferenceError};
use crate::pipeline::structuring::types::LlmClient;

// ──────────────────────────────────────────────
// Types
// ──────────────────────────────────────────────

/// How the extraction stage processes the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractionStrategy {
    /// Vision model renders PDF pages → images → text via OCR.
    VisionOcr { model: String },
    /// Pdfium native text extraction (digital PDF only, no model needed).
    PdfiumText,
    /// Direct UTF-8 read (plain text files).
    DirectText,
}

/// How pages flow through extraction → structuring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingMode {
    /// No model swap between stages.
    /// Used when same model handles both, or extraction needs no model.
    Interleaved,
    /// Extract ALL pages → swap models → structure ALL pages.
    /// Used when different models handle extraction vs structuring.
    BatchStages,
}

/// Complete pipeline assignment resolved from tags + fallbacks.
#[derive(Debug, Clone)]
pub struct PipelineAssignment {
    pub extraction: ExtractionStrategy,
    pub structuring_model: String,
    pub processing_mode: ProcessingMode,
    /// L6-05: Prompt strategy for this pipeline run. None = legacy behavior.
    pub prompt_strategy: Option<crate::pipeline::strategy::PromptStrategy>,
}

// ──────────────────────────────────────────────
// Resolution
// ──────────────────────────────────────────────

/// Resolve pipeline assignment from model tags, enabled state, and file category.
///
/// This is the single entry point for determining how a document is processed.
/// It replaces the ad-hoc model resolution in `commands/import.rs`.
pub fn resolve_pipeline(
    conn: &Connection,
    resolver: &ActiveModelResolver,
    client: &dyn LlmClient,
    file_category: &FileCategory,
) -> Result<PipelineAssignment, PreferenceError> {
    // Load all tags and disabled models from DB
    let all_tags = repository::get_all_model_tags(conn)?;
    let disabled = repository::get_disabled_models(conn)?;

    // Get installed models from Ollama
    let installed = client
        .list_models()
        .map_err(|e| PreferenceError::OllamaUnavailable(e.to_string()))?;

    // Filter to enabled-only
    let enabled = filter_enabled(&installed, &disabled);

    if enabled.is_empty() {
        return Err(PreferenceError::NoModelAvailable);
    }

    // Resolve each stage
    let extraction = resolve_extraction(file_category, &all_tags, &enabled)?;
    let structuring_model = resolve_structuring(conn, resolver, client, &all_tags, &enabled)?;

    // Auto-select processing mode
    // BTL-03: Use normalized identity to compare — namespaced variants
    // (e.g., "ktiyab/coheara-medgemma-4b-f16" vs "coheara-medgemma-4b-f16")
    // are the same model and should use Interleaved mode.
    let processing_mode = match &extraction {
        ExtractionStrategy::VisionOcr { model }
            if normalize_model_identity(model)
                != normalize_model_identity(&structuring_model) =>
        {
            ProcessingMode::BatchStages
        }
        _ => ProcessingMode::Interleaved,
    };

    Ok(PipelineAssignment {
        extraction,
        structuring_model,
        processing_mode,
        prompt_strategy: None, // L6-05: Caller can resolve and set after pipeline assignment
    })
}

// ──────────────────────────────────────────────
// Internal helpers
// ──────────────────────────────────────────────

/// Resolve extraction strategy from file category + tags.
///
/// BTL-02: CT-01 capability tags are the sole authority.
/// Prefix heuristic removed — if no VISION tag, digital PDFs fall back to PdfiumText,
/// scanned PDFs and images return NoVisionModel error.
fn resolve_extraction(
    file_category: &FileCategory,
    all_tags: &HashMap<String, Vec<CapabilityTag>>,
    enabled: &[String],
) -> Result<ExtractionStrategy, PreferenceError> {
    match file_category {
        FileCategory::PlainText => Ok(ExtractionStrategy::DirectText),
        FileCategory::Unsupported => Err(PreferenceError::NoModelAvailable),

        FileCategory::DigitalPdf => {
            if let Some(model) = find_enabled_with_tag(all_tags, enabled, CapabilityTag::Vision) {
                Ok(ExtractionStrategy::VisionOcr { model })
            } else {
                tracing::info!("No VISION-tagged model — falling back to PdfiumText for digital PDF");
                Ok(ExtractionStrategy::PdfiumText)
            }
        }

        FileCategory::ScannedPdf | FileCategory::Image => {
            if let Some(model) = find_enabled_with_tag(all_tags, enabled, CapabilityTag::Vision) {
                Ok(ExtractionStrategy::VisionOcr { model })
            } else {
                Err(PreferenceError::NoVisionModel)
            }
        }
    }
}

/// Resolve structuring model from tags + preferences.
fn resolve_structuring(
    conn: &Connection,
    resolver: &ActiveModelResolver,
    client: &dyn LlmClient,
    all_tags: &HashMap<String, Vec<CapabilityTag>>,
    enabled: &[String],
) -> Result<String, PreferenceError> {
    // Step 1: User preference (if enabled and installed)
    if let Ok(resolved) = resolver.resolve(conn, client) {
        if enabled.iter().any(|m| m == &resolved.name) {
            return Ok(resolved.name);
        }
    }

    // Step 2: Enabled model with MEDICAL tag
    if let Some(model) = find_enabled_with_tag(all_tags, enabled, CapabilityTag::Medical) {
        return Ok(model);
    }

    // Step 3: Any enabled model (BTL-02: prefix heuristic removed, tags are sole authority)
    // classify_model() still exists for informational labeling but no longer gates routing.
    if let Some(model) = enabled.first() {
        return Ok(model.clone());
    }

    Err(PreferenceError::NoModelAvailable)
}

/// Find first enabled model with a specific tag.
fn find_enabled_with_tag(
    all_tags: &HashMap<String, Vec<CapabilityTag>>,
    enabled: &[String],
    tag: CapabilityTag,
) -> Option<String> {
    for model in enabled {
        if let Some(tags) = all_tags.get(model) {
            if tags.contains(&tag) {
                return Some(model.clone());
            }
        }
    }
    None
}

// BTL-02: find_enabled_vision_by_prefix() removed. CT-01 tags are sole authority.

/// Filter installed models to only those that are enabled.
fn filter_enabled(installed: &[String], disabled: &[String]) -> Vec<String> {
    installed
        .iter()
        .filter(|m| !disabled.contains(m))
        .cloned()
        .collect()
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;
    use crate::pipeline::structuring::StructuringError;

    struct MockClient {
        models: Vec<String>,
    }

    impl MockClient {
        fn with(models: &[&str]) -> Self {
            Self {
                models: models.iter().map(|s| s.to_string()).collect(),
            }
        }
    }

    impl LlmClient for MockClient {
        fn generate(&self, _m: &str, _p: &str, _s: &str) -> Result<String, StructuringError> {
            Ok(String::new())
        }
        fn is_model_available(&self, model: &str) -> Result<bool, StructuringError> {
            Ok(self.models.contains(&model.to_string()))
        }
        fn list_models(&self) -> Result<Vec<String>, StructuringError> {
            Ok(self.models.clone())
        }
    }

    fn setup() -> Connection {
        open_memory_database().expect("in-memory DB")
    }

    // ── Scenario A: Two models, correct tag routing ──

    #[test]
    fn scenario_a_two_models_relay() {
        let conn = setup();
        let client = MockClient::with(&["llava:13b", "medgemma:4b"]);
        let resolver = ActiveModelResolver::new();

        // Tag llava as vision, medgemma as medical
        repository::set_model_tags(&conn, "llava:13b", &[CapabilityTag::Vision, CapabilityTag::Png, CapabilityTag::Jpeg]).unwrap();
        repository::set_model_tags(&conn, "medgemma:4b", &[CapabilityTag::Medical, CapabilityTag::Txt]).unwrap();

        let assignment = resolve_pipeline(&conn, &resolver, &client, &FileCategory::ScannedPdf).unwrap();

        assert_eq!(assignment.extraction, ExtractionStrategy::VisionOcr { model: "llava:13b".to_string() });
        assert_eq!(assignment.structuring_model, "medgemma:4b");
        assert_eq!(assignment.processing_mode, ProcessingMode::BatchStages);
    }

    // ── Scenario B: Single model, both tags ──

    #[test]
    fn scenario_b_single_model_dual_role() {
        let conn = setup();
        let client = MockClient::with(&["medgemma:4b"]);
        let resolver = ActiveModelResolver::new();

        repository::set_model_tags(&conn, "medgemma:4b", &[
            CapabilityTag::Vision, CapabilityTag::Medical, CapabilityTag::Png, CapabilityTag::Jpeg, CapabilityTag::Txt,
        ]).unwrap();

        let assignment = resolve_pipeline(&conn, &resolver, &client, &FileCategory::ScannedPdf).unwrap();

        assert_eq!(assignment.extraction, ExtractionStrategy::VisionOcr { model: "medgemma:4b".to_string() });
        assert_eq!(assignment.structuring_model, "medgemma:4b");
        assert_eq!(assignment.processing_mode, ProcessingMode::Interleaved);
    }

    // ── Scenario C: TXT-only model + digital PDF → PdfiumText ──

    #[test]
    fn scenario_c_txt_only_digital_pdf() {
        let conn = setup();
        let client = MockClient::with(&["llama3:8b"]);
        let resolver = ActiveModelResolver::new();

        repository::set_model_tags(&conn, "llama3:8b", &[CapabilityTag::Txt]).unwrap();

        let assignment = resolve_pipeline(&conn, &resolver, &client, &FileCategory::DigitalPdf).unwrap();

        assert_eq!(assignment.extraction, ExtractionStrategy::PdfiumText);
        assert_eq!(assignment.structuring_model, "llama3:8b");
        assert_eq!(assignment.processing_mode, ProcessingMode::Interleaved);
    }

    // ── Scenario D: TXT-only model + scanned PDF → error ──

    #[test]
    fn scenario_d_txt_only_scanned_pdf_errors() {
        let conn = setup();
        let client = MockClient::with(&["llama3:8b"]);
        let resolver = ActiveModelResolver::new();

        repository::set_model_tags(&conn, "llama3:8b", &[CapabilityTag::Txt]).unwrap();

        // BTL-02: TXT-only model → NoVisionModel for scanned PDFs
        let result = resolve_pipeline(&conn, &resolver, &client, &FileCategory::ScannedPdf);
        assert!(matches!(result, Err(PreferenceError::NoVisionModel)));
    }

    // ── Scenario E: No tags → NoVisionModel error (BTL-02) ──

    #[test]
    fn scenario_e_no_tags_returns_no_vision_model() {
        let conn = setup();
        let client = MockClient::with(&["medgemma:4b"]);
        let resolver = ActiveModelResolver::new();

        // No tags set — CT-01 tags are sole authority, no prefix fallback
        let result = resolve_pipeline(&conn, &resolver, &client, &FileCategory::ScannedPdf);
        assert!(matches!(result, Err(PreferenceError::NoVisionModel)));
    }

    #[test]
    fn scenario_e_no_tags_digital_pdf_falls_back_to_pdfium_text() {
        let conn = setup();
        let client = MockClient::with(&["medgemma:4b"]);
        let resolver = ActiveModelResolver::new();

        // No tags — digital PDF gracefully falls back to PdfiumText
        let assignment = resolve_pipeline(&conn, &resolver, &client, &FileCategory::DigitalPdf).unwrap();
        assert_eq!(assignment.extraction, ExtractionStrategy::PdfiumText);
    }

    // ── Scenario F: Disabled model skipped ──

    #[test]
    fn scenario_f_disabled_model_skipped() {
        let conn = setup();
        let client = MockClient::with(&["llava:13b", "medgemma:4b"]);
        let resolver = ActiveModelResolver::new();

        repository::set_model_tags(&conn, "llava:13b", &[CapabilityTag::Vision]).unwrap();
        repository::set_model_tags(&conn, "medgemma:4b", &[CapabilityTag::Vision, CapabilityTag::Medical, CapabilityTag::Txt]).unwrap();

        // Disable llava
        repository::set_model_enabled(&conn, "llava:13b", false).unwrap();

        let assignment = resolve_pipeline(&conn, &resolver, &client, &FileCategory::ScannedPdf).unwrap();

        // llava is disabled, so medgemma handles both
        assert_eq!(assignment.extraction, ExtractionStrategy::VisionOcr { model: "medgemma:4b".to_string() });
        assert_eq!(assignment.structuring_model, "medgemma:4b");
        assert_eq!(assignment.processing_mode, ProcessingMode::Interleaved);
    }

    // ── Edge cases ──

    #[test]
    fn empty_models_returns_error() {
        let conn = setup();
        let client = MockClient::with(&[]);
        let resolver = ActiveModelResolver::new();

        let result = resolve_pipeline(&conn, &resolver, &client, &FileCategory::ScannedPdf);
        assert!(matches!(result, Err(PreferenceError::NoModelAvailable)));
    }

    #[test]
    fn all_models_disabled_returns_error() {
        let conn = setup();
        let client = MockClient::with(&["medgemma:4b"]);
        let resolver = ActiveModelResolver::new();

        repository::set_model_enabled(&conn, "medgemma:4b", false).unwrap();

        let result = resolve_pipeline(&conn, &resolver, &client, &FileCategory::ScannedPdf);
        assert!(matches!(result, Err(PreferenceError::NoModelAvailable)));
    }

    #[test]
    fn plain_text_uses_direct_text() {
        let conn = setup();
        let client = MockClient::with(&["llama3:8b"]);
        let resolver = ActiveModelResolver::new();

        let assignment = resolve_pipeline(&conn, &resolver, &client, &FileCategory::PlainText).unwrap();

        assert_eq!(assignment.extraction, ExtractionStrategy::DirectText);
        assert_eq!(assignment.structuring_model, "llama3:8b");
        assert_eq!(assignment.processing_mode, ProcessingMode::Interleaved);
    }

    #[test]
    fn image_requires_vision_model() {
        let conn = setup();
        let client = MockClient::with(&["llama3:8b"]);
        let resolver = ActiveModelResolver::new();

        repository::set_model_tags(&conn, "llama3:8b", &[CapabilityTag::Txt]).unwrap();

        // BTL-02: TXT-only model → NoVisionModel for images
        let result = resolve_pipeline(&conn, &resolver, &client, &FileCategory::Image);
        assert!(matches!(result, Err(PreferenceError::NoVisionModel)));
    }

    #[test]
    fn user_preference_respected_for_structuring() {
        let conn = setup();
        let client = MockClient::with(&["llava:13b", "medgemma:4b", "llama3:8b"]);
        let resolver = ActiveModelResolver::new();

        // User explicitly chose llama3
        crate::db::repository::set_model_preference(
            &conn,
            "llama3:8b",
            &super::super::structuring::preferences::ModelQuality::General,
            &crate::pipeline::structuring::preferences::PreferenceSource::User,
        ).unwrap();

        repository::set_model_tags(&conn, "llava:13b", &[CapabilityTag::Vision]).unwrap();
        repository::set_model_tags(&conn, "medgemma:4b", &[CapabilityTag::Medical]).unwrap();

        let assignment = resolve_pipeline(&conn, &resolver, &client, &FileCategory::ScannedPdf).unwrap();

        // User preference for structuring respected
        assert_eq!(assignment.structuring_model, "llama3:8b");
        // Vision model still used for extraction
        assert_eq!(assignment.extraction, ExtractionStrategy::VisionOcr { model: "llava:13b".to_string() });
    }

    #[test]
    fn digital_pdf_prefers_vision_if_available() {
        let conn = setup();
        let client = MockClient::with(&["medgemma:4b"]);
        let resolver = ActiveModelResolver::new();

        repository::set_model_tags(&conn, "medgemma:4b", &[CapabilityTag::Vision, CapabilityTag::Medical]).unwrap();

        let assignment = resolve_pipeline(&conn, &resolver, &client, &FileCategory::DigitalPdf).unwrap();

        // Vision model preferred even for digital PDFs (better extraction quality)
        assert_eq!(assignment.extraction, ExtractionStrategy::VisionOcr { model: "medgemma:4b".to_string() });
    }
}
