use super::types::{QueryType, RetrievalParams};

/// Classify a patient query into a type using keyword heuristics.
pub fn classify_query(text: &str) -> QueryType {
    let lower = text.to_lowercase();

    if has_timeline_pattern(&lower) {
        return QueryType::Timeline;
    }

    if has_symptom_pattern(&lower) {
        return QueryType::Symptom;
    }

    if has_exploratory_pattern(&lower) {
        return QueryType::Exploratory;
    }

    if has_factual_pattern(&lower) {
        return QueryType::Factual;
    }

    QueryType::General
}

/// Determine retrieval parameters based on query type.
pub fn retrieval_strategy(query_type: &QueryType) -> RetrievalParams {
    match query_type {
        QueryType::Factual => RetrievalParams {
            semantic_top_k: 5,
            include_medications: true,
            include_labs: true,
            include_diagnoses: true,
            include_allergies: true,
            include_symptoms: false,
            include_conversations: false,
            temporal_weight: 0.2,
        },
        QueryType::Exploratory => RetrievalParams {
            semantic_top_k: 8,
            include_medications: true,
            include_labs: true,
            include_diagnoses: true,
            include_allergies: true,
            include_symptoms: true,
            include_conversations: true,
            temporal_weight: 0.5,
        },
        QueryType::Symptom => RetrievalParams {
            semantic_top_k: 5,
            include_medications: true,
            include_labs: false,
            include_diagnoses: true,
            include_allergies: false,
            include_symptoms: true,
            include_conversations: false,
            temporal_weight: 0.7,
        },
        QueryType::Timeline => RetrievalParams {
            semantic_top_k: 3,
            include_medications: true,
            include_labs: true,
            include_diagnoses: true,
            include_allergies: false,
            include_symptoms: true,
            include_conversations: false,
            temporal_weight: 1.0,
        },
        QueryType::General => RetrievalParams {
            semantic_top_k: 5,
            include_medications: true,
            include_labs: false,
            include_diagnoses: true,
            include_allergies: false,
            include_symptoms: false,
            include_conversations: false,
            temporal_weight: 0.3,
        },
    }
}

fn has_timeline_pattern(text: &str) -> bool {
    let patterns = [
        "what changed",
        "what's changed",
        "since my last",
        "since last visit",
        "over the past",
        "history of",
        "when did",
        "how long",
        "timeline",
        "chronolog",
        "what happened",
        "evolution",
        "progression",
    ];
    patterns.iter().any(|p| text.contains(p))
}

fn has_symptom_pattern(text: &str) -> bool {
    let patterns = [
        "feeling",
        "symptom",
        "pain",
        "dizzy",
        "nausea",
        "headache",
        "tired",
        "fatigue",
        "side effect",
        "since i started",
        "after taking",
        "hurts",
        "uncomfortable",
        "worse",
        "better",
    ];
    patterns.iter().any(|p| text.contains(p))
}

fn has_exploratory_pattern(text: &str) -> bool {
    let patterns = [
        "what should i ask",
        "what questions",
        "prepare for",
        "before my appointment",
        "what to expect",
        "should i be concerned",
        "what does this mean",
        "help me understand",
    ];
    patterns.iter().any(|p| text.contains(p))
}

fn has_factual_pattern(text: &str) -> bool {
    let patterns = [
        "what is my",
        "what's my",
        "what dose",
        "how much",
        "how often",
        "who prescribed",
        "when was",
        "which doctor",
        "what medication",
        "what are my",
        "lab result",
        "test result",
    ];
    patterns.iter().any(|p| text.contains(p))
}

/// Extract medical keywords from a query for targeted SQLite lookups.
pub fn extract_medical_keywords(query: &str) -> Vec<String> {
    let words: Vec<&str> = query.split_whitespace().collect();
    let mut keywords = Vec::new();

    for word in &words {
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
        if clean.len() >= 3 {
            keywords.push(clean.to_lowercase());
        }
    }

    keywords
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_factual_queries() {
        assert_eq!(classify_query("What dose of metformin am I on?"), QueryType::Factual);
        assert_eq!(classify_query("What is my blood pressure?"), QueryType::Factual);
        assert_eq!(classify_query("What are my lab results?"), QueryType::Factual);
        assert_eq!(classify_query("How often do I take my medication?"), QueryType::Factual);
    }

    #[test]
    fn classify_timeline_queries() {
        assert_eq!(
            classify_query("What changed since my last visit?"),
            QueryType::Timeline
        );
        assert_eq!(
            classify_query("When did I start taking metformin?"),
            QueryType::Timeline
        );
        assert_eq!(
            classify_query("Show me the progression of my HbA1c"),
            QueryType::Timeline
        );
    }

    #[test]
    fn classify_symptom_queries() {
        assert_eq!(
            classify_query("I've been feeling dizzy lately"),
            QueryType::Symptom
        );
        assert_eq!(
            classify_query("I have a headache since I started the new medication"),
            QueryType::Symptom
        );
        assert_eq!(
            classify_query("My side effect is getting worse"),
            QueryType::Symptom
        );
    }

    #[test]
    fn classify_exploratory_queries() {
        assert_eq!(
            classify_query("What should I ask my doctor about?"),
            QueryType::Exploratory
        );
        assert_eq!(
            classify_query("Help me understand my lab results"),
            QueryType::Exploratory
        );
        assert_eq!(
            classify_query("What to expect at my appointment"),
            QueryType::Exploratory
        );
    }

    #[test]
    fn classify_general_queries() {
        assert_eq!(classify_query("Tell me about my health"), QueryType::General);
        assert_eq!(classify_query("Hello"), QueryType::General);
    }

    #[test]
    fn retrieval_strategy_factual_includes_meds_and_labs() {
        let params = retrieval_strategy(&QueryType::Factual);
        assert!(params.include_medications);
        assert!(params.include_labs);
        assert!(!params.include_symptoms);
    }

    #[test]
    fn retrieval_strategy_symptom_includes_symptoms() {
        let params = retrieval_strategy(&QueryType::Symptom);
        assert!(params.include_symptoms);
        assert!(params.include_medications);
        assert!(!params.include_labs);
    }

    #[test]
    fn retrieval_strategy_timeline_has_high_temporal_weight() {
        let params = retrieval_strategy(&QueryType::Timeline);
        assert_eq!(params.temporal_weight, 1.0);
    }

    #[test]
    fn extract_keywords_filters_short_words() {
        let keywords = extract_medical_keywords("What is my HbA1c level?");
        assert!(keywords.contains(&"what".to_string()));
        assert!(keywords.contains(&"hba1c".to_string()));
        assert!(keywords.contains(&"level".to_string()));
        // "is" and "my" are too short (< 3 chars)
        assert!(!keywords.contains(&"is".to_string()));
        assert!(!keywords.contains(&"my".to_string()));
    }

    #[test]
    fn extract_keywords_handles_punctuation() {
        let keywords = extract_medical_keywords("metformin, aspirin, and lisinopril");
        assert!(keywords.contains(&"metformin".to_string()));
        assert!(keywords.contains(&"aspirin".to_string()));
        assert!(keywords.contains(&"lisinopril".to_string()));
    }
}
