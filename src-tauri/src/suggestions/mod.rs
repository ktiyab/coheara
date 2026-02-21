//! LP-05: Intelligent chat suggestion scoring.
//!
//! Replaces the hardcoded 6-suggestion system with a `SuggestionScorer` that
//! queries actual health data and surfaces ranked, personalized suggestions.
//! Each health domain is an independent `SignalProvider`. No LLM calls — pure SQL.

pub mod appt_provider;
pub mod doc_provider;
pub mod lab_provider;
pub mod med_provider;
pub mod symptom_provider;

use std::cmp::Ordering;
use std::collections::HashMap;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::DatabaseError;
use crate::db::repository::get_recent_user_messages;

// ─── Public types ────────────────────────────────────────────────────────────

/// Intent determines frontend behavior on tap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SuggestionIntent {
    /// Tap → auto-send (user asks about their data).
    Query,
    /// Tap → pre-fill input (user completes the sentence, then sends).
    Expression,
}

/// Serialized to frontend via IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptSuggestion {
    pub template_key: String,
    pub params: HashMap<String, String>,
    pub category: String,
    pub intent: SuggestionIntent,
}

// ─── Internal types ──────────────────────────────────────────────────────────

/// A scored suggestion candidate produced by a SignalProvider.
#[derive(Debug, Clone)]
pub struct ScoredSuggestion {
    pub template_key: String,
    pub params: HashMap<String, String>,
    pub intent: SuggestionIntent,
    pub score: f32,
    pub domain: &'static str,
    pub entity_id: Option<String>,
    pub category: String,
}

impl ScoredSuggestion {
    fn into_prompt(self) -> PromptSuggestion {
        PromptSuggestion {
            template_key: self.template_key,
            params: self.params,
            category: self.category,
            intent: self.intent,
        }
    }
}

// ─── Trait ────────────────────────────────────────────────────────────────────

/// One per health domain. Self-contained, independently testable.
pub trait SignalProvider: Send + Sync {
    /// Query the database and return scored suggestion candidates.
    /// `recent_topics` is concatenated text of user messages from the last 48h,
    /// used to avoid suggesting things the user just discussed.
    fn collect(
        &self,
        conn: &Connection,
        recent_topics: &str,
    ) -> Result<Vec<ScoredSuggestion>, DatabaseError>;
}

// ─── Scoring primitives ──────────────────────────────────────────────────────

/// Decays from 1.0 to 0.0 over `window` days.
/// Item from today = 1.0, item from `window` days ago = 0.0.
pub fn recency_decay(days_since: f32, window: f32) -> f32 {
    (1.0 - days_since / window).max(0.0)
}

/// Grows from 0.0 to 1.0 over `window` days since last mention.
/// Never discussed = 1.0, discussed today = 0.0.
pub fn staleness(days_since_mention: f32, window: f32) -> f32 {
    (days_since_mention / window).min(1.0)
}

// ─── Static defaults ─────────────────────────────────────────────────────────

fn default_suggestions() -> Vec<PromptSuggestion> {
    vec![
        PromptSuggestion {
            template_key: "chat.default_ask_medications".into(),
            params: HashMap::new(),
            category: "medications".into(),
            intent: SuggestionIntent::Query,
        },
        PromptSuggestion {
            template_key: "chat.default_ask_lab_results".into(),
            params: HashMap::new(),
            category: "labs".into(),
            intent: SuggestionIntent::Query,
        },
        PromptSuggestion {
            template_key: "chat.default_ask_interactions".into(),
            params: HashMap::new(),
            category: "medications".into(),
            intent: SuggestionIntent::Query,
        },
        PromptSuggestion {
            template_key: "chat.default_ask_doctor".into(),
            params: HashMap::new(),
            category: "appointments".into(),
            intent: SuggestionIntent::Query,
        },
        PromptSuggestion {
            template_key: "chat.default_ask_diagnosis".into(),
            params: HashMap::new(),
            category: "general".into(),
            intent: SuggestionIntent::Query,
        },
        PromptSuggestion {
            template_key: "chat.default_ask_changes".into(),
            params: HashMap::new(),
            category: "general".into(),
            intent: SuggestionIntent::Query,
        },
    ]
}

fn expression_defaults() -> Vec<ScoredSuggestion> {
    vec![
        ScoredSuggestion {
            template_key: "chat.prefill_symptom".into(),
            params: HashMap::new(),
            intent: SuggestionIntent::Expression,
            score: 0.3,
            domain: "symptom",
            entity_id: None,
            category: "health".into(),
        },
        ScoredSuggestion {
            template_key: "chat.prefill_medication".into(),
            params: HashMap::new(),
            intent: SuggestionIntent::Expression,
            score: 0.3,
            domain: "medication",
            entity_id: None,
            category: "health".into(),
        },
        ScoredSuggestion {
            template_key: "chat.prefill_appointment".into(),
            params: HashMap::new(),
            intent: SuggestionIntent::Expression,
            score: 0.3,
            domain: "appointment",
            entity_id: None,
            category: "health".into(),
        },
    ]
}

// ─── Scorer ──────────────────────────────────────────────────────────────────

pub struct SuggestionScorer {
    providers: Vec<Box<dyn SignalProvider>>,
}

impl SuggestionScorer {
    pub fn new() -> Self {
        Self {
            providers: vec![
                Box::new(lab_provider::LabSignalProvider),
                Box::new(med_provider::MedSignalProvider),
                Box::new(symptom_provider::SymptomSignalProvider),
                Box::new(appt_provider::ApptSignalProvider),
                Box::new(doc_provider::DocSignalProvider),
            ],
        }
    }

    pub fn score(
        &self,
        conn: &Connection,
        max_results: usize,
    ) -> Result<Vec<PromptSuggestion>, DatabaseError> {
        // Fetch recent topics once for all providers
        let recent_topics = get_recent_user_messages(conn, 48).unwrap_or_default();

        let mut candidates: Vec<ScoredSuggestion> = Vec::new();

        // Collect from each provider (resilient — warn and continue on failure)
        for provider in &self.providers {
            match provider.collect(conn, &recent_topics) {
                Ok(mut results) => candidates.append(&mut results),
                Err(e) => {
                    tracing::warn!("SignalProvider failed: {e}");
                }
            }
        }

        // Inject expression defaults
        candidates.extend(expression_defaults());

        // Deduplicate by (domain, entity_id) — keep highest score
        dedup_by_entity(&mut candidates);

        // Sort by score descending
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(Ordering::Equal)
        });

        // Take top N
        let mut results: Vec<PromptSuggestion> = candidates
            .into_iter()
            .take(max_results)
            .map(|s| s.into_prompt())
            .collect();

        // Pad with static defaults if needed
        if results.len() < max_results {
            let defaults = default_suggestions();
            for d in defaults {
                if results.len() >= max_results {
                    break;
                }
                if !results.iter().any(|r| r.template_key == d.template_key) {
                    results.push(d);
                }
            }
        }

        Ok(results)
    }
}

fn dedup_by_entity(candidates: &mut Vec<ScoredSuggestion>) {
    // Sort by (domain, entity_id, score DESC) then dedup keeping first (highest score)
    candidates.sort_by(|a, b| {
        a.domain
            .cmp(b.domain)
            .then_with(|| a.entity_id.cmp(&b.entity_id))
            .then_with(|| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(Ordering::Equal)
            })
    });
    candidates.dedup_by(|a, b| {
        a.domain == b.domain
            && a.entity_id.is_some()
            && a.entity_id == b.entity_id
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;

    #[test]
    fn scorer_empty_vault_returns_six() {
        let conn = open_memory_database().unwrap();
        let scorer = SuggestionScorer::new();
        let results = scorer.score(&conn, 6).unwrap();
        assert_eq!(results.len(), 6);
        // Should have both query and expression intents
        assert!(results.iter().any(|s| s.intent == SuggestionIntent::Query));
        assert!(results
            .iter()
            .any(|s| s.intent == SuggestionIntent::Expression));
    }

    #[test]
    fn scorer_all_template_keys_non_empty() {
        let conn = open_memory_database().unwrap();
        let scorer = SuggestionScorer::new();
        let results = scorer.score(&conn, 6).unwrap();
        assert!(results.iter().all(|s| !s.template_key.is_empty()));
        assert!(results.iter().all(|s| !s.category.is_empty()));
    }

    #[test]
    fn recency_decay_boundaries() {
        assert!((recency_decay(0.0, 14.0) - 1.0).abs() < f32::EPSILON);
        assert!((recency_decay(14.0, 14.0) - 0.0).abs() < f32::EPSILON);
        assert!((recency_decay(7.0, 14.0) - 0.5).abs() < f32::EPSILON);
        assert!((recency_decay(20.0, 14.0) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn staleness_boundaries() {
        assert!((staleness(0.0, 7.0) - 0.0).abs() < f32::EPSILON);
        assert!((staleness(7.0, 7.0) - 1.0).abs() < f32::EPSILON);
        assert!((staleness(3.5, 7.0) - 0.5).abs() < f32::EPSILON);
        assert!((staleness(14.0, 7.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn dedup_keeps_highest_score() {
        let mut candidates = vec![
            ScoredSuggestion {
                template_key: "a".into(),
                params: HashMap::new(),
                intent: SuggestionIntent::Query,
                score: 0.5,
                domain: "lab",
                entity_id: Some("creatinine".into()),
                category: "labs".into(),
            },
            ScoredSuggestion {
                template_key: "b".into(),
                params: HashMap::new(),
                intent: SuggestionIntent::Query,
                score: 0.8,
                domain: "lab",
                entity_id: Some("creatinine".into()),
                category: "labs".into(),
            },
        ];
        dedup_by_entity(&mut candidates);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].template_key, "b");
    }
}
