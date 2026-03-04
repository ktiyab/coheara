//! ME-01 Brick 2: BM25+ scoring engine for medical items.
//!
//! Implements BM25+ (Lv & Zhai 2011) on medical item text fields.
//! BM25+ adds a lower-bound term frequency component (delta=1.0) so
//! that long documents with even one occurrence of a query term are
//! never penalized below zero.
//!
//! Also provides query expansion via a medical synonym map.

use std::collections::HashMap;

use super::medical_item::MedicalItem;

/// BM25+ parameters (standard defaults from literature).
const K1: f32 = 1.2;
const B: f32 = 0.75;
const DELTA: f32 = 1.0;

/// Score all items against a query using BM25+.
///
/// Returns a parallel vec of scores (same order as `items`).
/// Scores are non-negative. Zero means no query term matched.
pub fn bm25_score(query: &str, items: &[MedicalItem]) -> Vec<f32> {
    if items.is_empty() {
        return vec![];
    }

    let query_terms = tokenize_and_expand(query);
    if query_terms.is_empty() {
        return vec![0.0; items.len()];
    }

    // Pre-tokenize all items.
    let item_tokens: Vec<Vec<String>> = items
        .iter()
        .map(|item| tokenize(&item.searchable_text))
        .collect();

    // Average document length.
    let total_len: usize = item_tokens.iter().map(|t| t.len()).sum();
    let avg_dl = total_len as f32 / items.len() as f32;

    // IDF per query term: log((N - df + 0.5) / (df + 0.5) + 1)
    let n = items.len() as f32;
    let mut idf: HashMap<&str, f32> = HashMap::new();
    for qt in &query_terms {
        let df = item_tokens
            .iter()
            .filter(|tokens| tokens.iter().any(|t| t == qt))
            .count() as f32;
        let idf_val = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
        idf.insert(qt.as_str(), idf_val.max(0.0));
    }

    // Score each item.
    item_tokens
        .iter()
        .map(|tokens| {
            let dl = tokens.len() as f32;
            let mut score = 0.0f32;

            for qt in &query_terms {
                let tf = tokens.iter().filter(|t| *t == qt).count() as f32;
                if tf == 0.0 {
                    continue;
                }
                let idf_val = idf.get(qt.as_str()).copied().unwrap_or(0.0);
                // BM25+ formula
                let numerator = tf * (K1 + 1.0);
                let denominator = tf + K1 * (1.0 - B + B * dl / avg_dl);
                score += idf_val * (numerator / denominator + DELTA);
            }
            score
        })
        .collect()
}

/// Normalize BM25 scores to [0, 1] range.
/// Returns 0.0 for all items if max_score is 0.
pub fn normalize_scores(scores: &[f32]) -> Vec<f32> {
    let max_score = scores.iter().cloned().fold(0.0f32, f32::max);
    if max_score <= 0.0 {
        return vec![0.0; scores.len()];
    }
    scores.iter().map(|s| s / max_score).collect()
}

/// Tokenize text into lowercase terms, filtering stopwords and short tokens.
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 2)
        .map(|w| w.to_lowercase())
        .filter(|w| !is_stopword(w))
        .collect()
}

/// Tokenize a query and expand with medical synonyms.
fn tokenize_and_expand(query: &str) -> Vec<String> {
    let base_tokens = tokenize(query);
    let mut expanded = base_tokens.clone();

    for token in &base_tokens {
        if let Some(synonyms) = medical_synonyms(token) {
            for syn in synonyms {
                if !expanded.contains(&syn.to_string()) {
                    expanded.push(syn.to_string());
                }
            }
        }
    }
    expanded
}

/// Common medical term synonyms for query expansion.
/// Returns alternative terms the patient might use vs. clinical names.
fn medical_synonyms(term: &str) -> Option<&'static [&'static str]> {
    match term {
        // Generic ↔ brand name mappings
        "metformin" => Some(&["glucophage"]),
        "glucophage" => Some(&["metformin"]),
        "warfarin" => Some(&["coumadin"]),
        "coumadin" => Some(&["warfarin"]),
        "ibuprofen" => Some(&["advil", "motrin"]),
        "advil" | "motrin" => Some(&["ibuprofen"]),
        "acetaminophen" | "paracetamol" => Some(&["tylenol", "paracetamol", "acetaminophen"]),
        "tylenol" => Some(&["acetaminophen", "paracetamol"]),
        "lisinopril" => Some(&["zestril", "prinivil"]),
        "atorvastatin" => Some(&["lipitor"]),
        "lipitor" => Some(&["atorvastatin"]),
        "omeprazole" => Some(&["prilosec"]),
        "amlodipine" => Some(&["norvasc"]),

        // Patient language ↔ clinical terms
        "sugar" => Some(&["glucose", "glycemia", "glycemie"]),
        "glucose" | "glycemia" | "glycemie" => Some(&["sugar"]),
        "pressure" | "bp" => Some(&["blood pressure", "hypertension"]),
        "cholesterol" => Some(&["lipid", "ldl", "hdl"]),
        "ldl" | "hdl" => Some(&["cholesterol", "lipid"]),
        "kidney" | "renal" => Some(&["creatinine", "gfr", "kidney", "renal"]),
        "creatinine" | "gfr" => Some(&["kidney", "renal"]),
        "thyroid" => Some(&["tsh", "t3", "t4"]),
        "tsh" => Some(&["thyroid"]),
        "iron" => Some(&["ferritin", "hemoglobin"]),
        "ferritin" => Some(&["iron"]),
        "heart" | "cardiac" => Some(&["cardiovascular", "heart", "cardiac"]),
        "liver" | "hepatic" => Some(&["alt", "ast", "bilirubin", "liver", "hepatic"]),
        "alt" | "ast" | "bilirubin" => Some(&["liver", "hepatic"]),

        // French patient language
        "tension" => Some(&["blood pressure", "hypertension", "pression"]),
        "sucre" => Some(&["glucose", "glycemie"]),
        "rein" | "reins" => Some(&["creatinine", "kidney"]),
        "foie" => Some(&["liver", "hepatic", "alt", "ast"]),
        "coeur" => Some(&["heart", "cardiac", "cardiovascular"]),

        _ => None,
    }
}

/// Stopwords filtered from tokenization (EN + FR).
fn is_stopword(word: &str) -> bool {
    matches!(
        word,
        "the" | "is" | "am" | "are" | "was" | "were" | "be" | "been"
            | "my" | "me" | "of" | "on" | "in" | "to" | "for" | "and"
            | "or" | "an" | "it" | "do" | "at" | "by" | "so" | "if"
            | "le" | "la" | "les" | "de" | "du" | "des" | "un" | "une"
            | "et" | "ou" | "en" | "au" | "aux" | "je" | "ce" | "que"
            | "qui" | "est" | "mon" | "ma" | "mes" | "son" | "sa" | "ses"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::rag::medical_item::{ItemType, SeveritySignal, StatusSignal};
    use chrono::NaiveDate;
    use uuid::Uuid;

    fn make_item(name: &str, searchable: &str, item_type: ItemType) -> MedicalItem {
        MedicalItem {
            id: Uuid::new_v4(),
            item_type,
            display_name: name.into(),
            searchable_text: searchable.into(),
            document_id: Some(Uuid::new_v4()),
            relevant_date: Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
            severity: SeveritySignal::None,
            status: StatusSignal::Active,
        }
    }

    #[test]
    fn exact_match_scores_highest() {
        let items = vec![
            make_item("Metformin", "Metformin Glucophage 500mg Type 2 Diabetes", ItemType::Medication),
            make_item("Aspirin", "Aspirin 100mg cardiovascular", ItemType::Medication),
            make_item("HbA1c", "HbA1c 7.2 4548-4", ItemType::LabResult),
        ];
        let scores = bm25_score("metformin", &items);
        assert!(scores[0] > scores[1], "Metformin should score higher than Aspirin");
        assert!(scores[0] > scores[2], "Metformin should score higher than HbA1c");
    }

    #[test]
    fn no_match_scores_zero() {
        let items = vec![
            make_item("Aspirin", "Aspirin 100mg", ItemType::Medication),
        ];
        let scores = bm25_score("xyz_nonexistent", &items);
        assert_eq!(scores[0], 0.0);
    }

    #[test]
    fn empty_query_scores_zero() {
        let items = vec![
            make_item("Aspirin", "Aspirin 100mg", ItemType::Medication),
        ];
        let scores = bm25_score("", &items);
        assert_eq!(scores[0], 0.0);
    }

    #[test]
    fn empty_items_returns_empty() {
        let scores = bm25_score("metformin", &[]);
        assert!(scores.is_empty());
    }

    #[test]
    fn query_expansion_finds_synonyms() {
        let items = vec![
            make_item("Glucophage", "Glucophage 500mg", ItemType::Medication),
        ];
        // "metformin" expands to include "glucophage"
        let scores = bm25_score("metformin", &items);
        assert!(scores[0] > 0.0, "Synonym expansion should match Glucophage");
    }

    #[test]
    fn patient_language_expansion() {
        let items = vec![
            make_item("Blood Glucose", "glucose 120 mg/dL", ItemType::LabResult),
        ];
        // "sugar" expands to include "glucose"
        let scores = bm25_score("sugar level", &items);
        assert!(scores[0] > 0.0, "Patient term 'sugar' should match 'glucose'");
    }

    #[test]
    fn normalize_scales_to_unit() {
        let scores = vec![0.0, 2.5, 5.0, 1.0];
        let normalized = normalize_scores(&scores);
        assert_eq!(normalized[2], 1.0);
        assert_eq!(normalized[0], 0.0);
        assert!((normalized[1] - 0.5).abs() < 0.01);
    }

    #[test]
    fn normalize_all_zero_stays_zero() {
        let scores = vec![0.0, 0.0, 0.0];
        let normalized = normalize_scores(&scores);
        assert!(normalized.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn multi_term_query_additive() {
        let items = vec![
            make_item("Metformin", "Metformin Glucophage 500mg Type 2 Diabetes", ItemType::Medication),
            make_item("Type 2 Diabetes", "Type 2 Diabetes", ItemType::Diagnosis),
        ];
        // Both terms appear in item[0], only one in item[1]
        let scores = bm25_score("metformin diabetes", &items);
        assert!(scores[0] > scores[1], "Item matching both terms should score higher");
    }

    #[test]
    fn french_query_works() {
        let items = vec![
            make_item("Blood Glucose", "glucose glycemie 5.8", ItemType::LabResult),
        ];
        let scores = bm25_score("sucre", &items);
        // "sucre" expands to "glucose", "glycemie"
        assert!(scores[0] > 0.0, "French term 'sucre' should match via expansion");
    }

    #[test]
    fn stopwords_filtered() {
        let tokens = tokenize("What is my blood pressure?");
        assert!(!tokens.contains(&"is".to_string()));
        assert!(!tokens.contains(&"my".to_string()));
        assert!(tokens.contains(&"blood".to_string()));
        assert!(tokens.contains(&"pressure".to_string()));
    }
}
