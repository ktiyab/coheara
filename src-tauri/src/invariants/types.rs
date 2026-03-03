//! ME-03: Core types for the Invariant Reference Engine.
//!
//! All types are language-independent for computation.
//! Display uses `InvariantLabel` for i18n (EN/FR/DE).

use uuid::Uuid;

/// Trilingual display label for invariant classifications.
///
/// Computation uses `key` (language-independent).
/// Display uses `get(lang)` to resolve the user's language.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InvariantLabel {
    pub key: &'static str,
    pub en: &'static str,
    pub fr: &'static str,
    pub de: &'static str,
}

impl InvariantLabel {
    /// Get the display text for a language code ("en", "fr", "de").
    /// Falls back to English for unknown languages.
    pub fn get(&self, lang: &str) -> &'static str {
        match lang {
            "fr" => self.fr,
            "de" => self.de,
            _ => self.en,
        }
    }
}

/// Kind of clinical insight produced by the enrichment engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsightKind {
    /// Vital sign or lab classified against clinical thresholds.
    Classification,
    /// Drug-drug interaction detected via family matching.
    Interaction,
    /// Allergen-drug cross-reactivity detected.
    CrossReactivity,
    /// Required monitoring lab missing or overdue.
    MissingMonitoring,
    /// Age/sex-triggered screening overdue.
    ScreeningDue,
    /// Lab or vital trend crossing clinical threshold.
    AbnormalTrend,
}

/// Severity of a clinical insight (determines priority in context assembly).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InsightSeverity {
    /// Informational — no action required.
    Info,
    /// Warning — review recommended.
    Warning,
    /// Critical — requires attention.
    Critical,
}

impl InsightSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            InsightSeverity::Info => "INFO",
            InsightSeverity::Warning => "WARNING",
            InsightSeverity::Critical => "CRITICAL",
        }
    }
}

/// Meaning equation factor contributions from a single insight.
///
/// These feed `M = D × R × V × T × S × (1-U)` in ME-01.
/// This struct stores the per-insight factors: D, T, S, U.
/// R (domain Relevance) and V (Validity) are computed at enrichment time
/// from the full context, not stored per individual insight.
#[derive(Debug, Clone, PartialEq)]
pub struct MeaningFactors {
    /// D: cross-domain connection multiplier (1.0 = single domain, 1.2-1.5 = cross-domain).
    pub domain_boost: f64,
    /// T: freshness within clinical monitoring interval (0.0 = overdue, 1.0 = fresh).
    pub temporal_weight: f64,
    /// S: clinical severity score (0.0-2.0 range, higher = more significant).
    pub significance: f64,
    /// U: uncertainty increase from missing data (0.0 = complete, positive = gaps).
    pub uncertainty_delta: f64,
}

impl Default for MeaningFactors {
    fn default() -> Self {
        Self {
            domain_boost: 1.0,
            temporal_weight: 1.0,
            significance: 1.0,
            uncertainty_delta: 0.0,
        }
    }
}

/// A single clinical insight produced by the enrichment engine.
///
/// Deterministic — no LLM involved. Computed from user data + invariant registry.
#[derive(Debug, Clone)]
pub struct ClinicalInsight {
    pub kind: InsightKind,
    pub severity: InsightSeverity,
    /// Machine key for the insight (e.g., "bp_grade_1_htn", "warfarin_aspirin_interaction").
    pub summary_key: String,
    /// Trilingual display description.
    pub description: InvariantLabel,
    /// Clinical guideline source (e.g., "ISH 2020", "KDIGO 2024").
    /// String (not &'static str) because bundled tier sources come from JSON.
    pub source: String,
    /// Entity IDs that triggered this insight.
    pub related_entities: Vec<Uuid>,
    /// Meaning equation factor contributions.
    pub meaning_factors: MeaningFactors,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_get_returns_correct_language() {
        let label = InvariantLabel {
            key: "grade_1_htn",
            en: "Grade 1 Hypertension",
            fr: "Hypertension de grade 1",
            de: "Hypertonie Grad 1",
        };
        assert_eq!(label.get("en"), "Grade 1 Hypertension");
        assert_eq!(label.get("fr"), "Hypertension de grade 1");
        assert_eq!(label.get("de"), "Hypertonie Grad 1");
    }

    #[test]
    fn label_get_falls_back_to_english() {
        let label = InvariantLabel {
            key: "test",
            en: "English",
            fr: "Français",
            de: "Deutsch",
        };
        assert_eq!(label.get("es"), "English");
        assert_eq!(label.get(""), "English");
        assert_eq!(label.get("ja"), "English");
    }

    #[test]
    fn severity_ordering() {
        assert!(InsightSeverity::Info < InsightSeverity::Warning);
        assert!(InsightSeverity::Warning < InsightSeverity::Critical);
    }

    #[test]
    fn severity_as_str() {
        assert_eq!(InsightSeverity::Info.as_str(), "INFO");
        assert_eq!(InsightSeverity::Warning.as_str(), "WARNING");
        assert_eq!(InsightSeverity::Critical.as_str(), "CRITICAL");
    }

    #[test]
    fn meaning_factors_default() {
        let mf = MeaningFactors::default();
        assert!((mf.domain_boost - 1.0).abs() < f64::EPSILON);
        assert!((mf.temporal_weight - 1.0).abs() < f64::EPSILON);
        assert!((mf.significance - 1.0).abs() < f64::EPSILON);
        assert!((mf.uncertainty_delta - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn insight_kind_equality() {
        assert_eq!(InsightKind::Classification, InsightKind::Classification);
        assert_ne!(InsightKind::Classification, InsightKind::Interaction);
        assert_ne!(InsightKind::CrossReactivity, InsightKind::MissingMonitoring);
    }

    #[test]
    fn clinical_insight_construction() {
        let insight = ClinicalInsight {
            kind: InsightKind::Classification,
            severity: InsightSeverity::Warning,
            summary_key: "bp_grade_1_htn".to_string(),
            description: InvariantLabel {
                key: "grade_1_htn",
                en: "Grade 1 Hypertension (140-159/90-99 mmHg)",
                fr: "Hypertension de grade 1 (140-159/90-99 mmHg)",
                de: "Hypertonie Grad 1 (140-159/90-99 mmHg)",
            },
            source: "ISH 2020".to_string(),
            related_entities: vec![Uuid::nil()],
            meaning_factors: MeaningFactors {
                domain_boost: 1.0,
                temporal_weight: 0.9,
                significance: 0.6,
                uncertainty_delta: 0.0,
            },
        };
        assert_eq!(insight.kind, InsightKind::Classification);
        assert_eq!(insight.severity, InsightSeverity::Warning);
        assert_eq!(insight.source, "ISH 2020");
        assert_eq!(insight.related_entities.len(), 1);
    }
}
