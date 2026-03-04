//! ME-01 Brick 1: Query domain classification and cross-domain relevance matrix.
//!
//! Classifies patient queries into medical domains (8 types) and provides
//! the D(item_type, query_domain) factor — how relevant each entity type is
//! to the detected query domain.

use super::medical_item::ItemType;

/// Medical query domain (8 variants, extends the original 5 QueryTypes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryDomain {
    Medication,
    Lab,
    Symptom,
    Diagnosis,
    Allergy,
    Procedure,
    Timeline,
    General,
}

/// Classify a patient query into a medical domain using keyword heuristics.
/// Extends the existing `classify_query` with domain-level granularity.
pub fn classify_domain(text: &str) -> QueryDomain {
    let lower = text.to_lowercase();

    // Allergy queries (check first — safety-critical)
    if has_allergy_pattern(&lower) {
        return QueryDomain::Allergy;
    }

    // Timeline queries
    if has_timeline_pattern(&lower) {
        return QueryDomain::Timeline;
    }

    // Lab queries
    if has_lab_pattern(&lower) {
        return QueryDomain::Lab;
    }

    // Symptom queries
    if has_symptom_pattern(&lower) {
        return QueryDomain::Symptom;
    }

    // Medication queries
    if has_medication_pattern(&lower) {
        return QueryDomain::Medication;
    }

    // Diagnosis queries
    if has_diagnosis_pattern(&lower) {
        return QueryDomain::Diagnosis;
    }

    // Procedure queries
    if has_procedure_pattern(&lower) {
        return QueryDomain::Procedure;
    }

    QueryDomain::General
}

/// Cross-domain relevance factor D(item_type, query_domain).
///
/// Returns a multiplier in [0.2, 2.0] indicating how relevant an entity type
/// is for a given query domain. Safety-critical connections score high:
/// e.g., allergy items score 2.0 for medication queries (interaction risk).
///
/// Matrix from ME-01 spec Table 3.
pub fn domain_relevance(item_type: ItemType, query_domain: QueryDomain) -> f32 {
    match (item_type, query_domain) {
        // ── Medication queries ────────────────────────────────────
        (ItemType::Medication, QueryDomain::Medication) => 2.0,
        (ItemType::Allergy, QueryDomain::Medication) => 2.0,  // Safety-critical
        (ItemType::Diagnosis, QueryDomain::Medication) => 1.5, // PrescribedFor link
        (ItemType::LabResult, QueryDomain::Medication) => 1.2, // MonitorsFor link
        (ItemType::Symptom, QueryDomain::Medication) => 0.8,   // Side effects
        (ItemType::VitalSign, QueryDomain::Medication) => 0.5, // Indirect

        // ── Lab queries ──────────────────────────────────────────
        (ItemType::LabResult, QueryDomain::Lab) => 2.0,
        (ItemType::Diagnosis, QueryDomain::Lab) => 1.5,   // EvidencesFor link
        (ItemType::Medication, QueryDomain::Lab) => 1.2,   // MonitorsFor link
        (ItemType::VitalSign, QueryDomain::Lab) => 1.0,    // Related metrics
        (ItemType::Allergy, QueryDomain::Lab) => 0.3,
        (ItemType::Symptom, QueryDomain::Lab) => 0.5,

        // ── Symptom queries ──────────────────────────────────────
        (ItemType::Symptom, QueryDomain::Symptom) => 2.0,
        (ItemType::Medication, QueryDomain::Symptom) => 1.5, // Side effects
        (ItemType::Diagnosis, QueryDomain::Symptom) => 1.5,  // Condition symptoms
        (ItemType::VitalSign, QueryDomain::Symptom) => 1.0,
        (ItemType::LabResult, QueryDomain::Symptom) => 0.8,
        (ItemType::Allergy, QueryDomain::Symptom) => 1.2,    // Allergic reactions

        // ── Diagnosis queries ────────────────────────────────────
        (ItemType::Diagnosis, QueryDomain::Diagnosis) => 2.0,
        (ItemType::Medication, QueryDomain::Diagnosis) => 1.5, // Treatment
        (ItemType::LabResult, QueryDomain::Diagnosis) => 1.5,  // Evidence
        (ItemType::Symptom, QueryDomain::Diagnosis) => 1.2,    // Presentation
        (ItemType::Allergy, QueryDomain::Diagnosis) => 0.5,
        (ItemType::VitalSign, QueryDomain::Diagnosis) => 0.8,

        // ── Allergy queries ──────────────────────────────────────
        (ItemType::Allergy, QueryDomain::Allergy) => 2.0,
        (ItemType::Medication, QueryDomain::Allergy) => 1.8, // Contraindications
        (ItemType::Symptom, QueryDomain::Allergy) => 1.2,    // Reactions
        (ItemType::Diagnosis, QueryDomain::Allergy) => 0.5,
        (ItemType::LabResult, QueryDomain::Allergy) => 0.3,
        (ItemType::VitalSign, QueryDomain::Allergy) => 0.2,

        // ── Procedure queries ────────────────────────────────────
        (ItemType::Diagnosis, QueryDomain::Procedure) => 1.5,
        (ItemType::Medication, QueryDomain::Procedure) => 1.0,
        (ItemType::LabResult, QueryDomain::Procedure) => 1.0,
        (ItemType::Allergy, QueryDomain::Procedure) => 1.5,   // Pre-op safety
        (ItemType::Symptom, QueryDomain::Procedure) => 0.5,
        (ItemType::VitalSign, QueryDomain::Procedure) => 0.8,

        // ── Timeline queries ─────────────────────────────────────
        // All types are equally relevant for timeline; temporal decay (T) will
        // differentiate. Use moderate base relevance.
        (_, QueryDomain::Timeline) => 1.0,

        // ── General queries ──────────────────────────────────────
        // Equal-weight baseline; the R factor (BM25+graph) does the work.
        (_, QueryDomain::General) => 1.0,
    }
}

// ── Keyword patterns ──────────────────────────────────────────────

fn has_allergy_pattern(text: &str) -> bool {
    let patterns = [
        // English
        "allerg", "allergic", "reaction to", "intolerant", "intolerance",
        "anaphyla", "contraindicated", "can i take",
        // French
        "allergi", "allergique", "reaction", "réaction", "intoleran",
        "anaphyla", "contre-indiq",
    ];
    patterns.iter().any(|p| text.contains(p))
}

fn has_lab_pattern(text: &str) -> bool {
    let patterns = [
        // English
        "lab result", "test result", "blood test", "blood work",
        "hba1c", "hemoglobin", "cholesterol", "creatinine", "glucose",
        "potassium", "sodium", "thyroid", "tsh", "ferritin",
        "reference range", "normal range",
        // French
        "resultat", "résultat", "analyse", "bilan sanguin", "prise de sang",
        "hemoglobine", "hémoglobine", "glycemie", "glycémie",
        "cholesterol", "cholestérol", "creatinine", "créatinine",
    ];
    patterns.iter().any(|p| text.contains(p))
}

fn has_symptom_pattern(text: &str) -> bool {
    let patterns = [
        // English
        "feeling", "symptom", "pain", "dizzy", "nausea", "headache",
        "tired", "fatigue", "side effect", "since i started", "after taking",
        "hurts", "uncomfortable", "worse", "better",
        // French
        "je me sens", "je ressens", "symptome", "symptôme", "douleur",
        "vertige", "nausee", "nausée", "mal de tete", "mal de tête",
        "fatigue", "effet secondaire", "effet indésirable",
        "depuis que je prends", "j'ai mal",
    ];
    patterns.iter().any(|p| text.contains(p))
}

fn has_medication_pattern(text: &str) -> bool {
    let patterns = [
        // English
        "medication", "medicine", "drug", "dose", "dosage", "prescri",
        "taking", "pill", "tablet", "capsule", "refill",
        "what am i taking", "why am i taking", "how often",
        // French
        "medicament", "médicament", "traitement", "dose", "posologie",
        "prescri", "comprime", "comprimé", "gelule", "gélule",
        "renouvellement",
    ];
    patterns.iter().any(|p| text.contains(p))
}

fn has_diagnosis_pattern(text: &str) -> bool {
    let patterns = [
        // English
        "diagnosis", "diagnosed", "condition", "disease", "disorder",
        "what do i have", "what's wrong",
        // French
        "diagnostic", "diagnostiqué", "maladie", "pathologie",
        "qu'est-ce que j'ai",
    ];
    patterns.iter().any(|p| text.contains(p))
}

fn has_procedure_pattern(text: &str) -> bool {
    let patterns = [
        // English
        "surgery", "procedure", "operation", "biopsy", "scan",
        "mri", "ct scan", "x-ray", "xray", "ultrasound", "endoscopy",
        // French
        "chirurgie", "intervention", "operation", "opération",
        "biopsie", "irm", "scanner", "radiographie", "echographie",
        "échographie",
    ];
    patterns.iter().any(|p| text.contains(p))
}

fn has_timeline_pattern(text: &str) -> bool {
    let patterns = [
        // English
        "what changed", "since my last", "over the past", "history of",
        "when did", "how long", "timeline", "chronolog", "what happened",
        "progression",
        // French
        "qu'est-ce qui a chang", "depuis ma derni", "historique",
        "quand est-ce", "depuis combien", "chronologie",
        "que s'est-il pass",
    ];
    patterns.iter().any(|p| text.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Domain classification ─────────────────────────────────────

    #[test]
    fn classify_medication_queries() {
        assert_eq!(classify_domain("What dose of metformin am I on?"), QueryDomain::Medication);
        assert_eq!(classify_domain("Why am I taking this medication?"), QueryDomain::Medication);
        assert_eq!(classify_domain("Quelle dose de metformine je prends?"), QueryDomain::Medication);
    }

    #[test]
    fn classify_lab_queries() {
        assert_eq!(classify_domain("What are my lab results?"), QueryDomain::Lab);
        assert_eq!(classify_domain("Is my HbA1c normal?"), QueryDomain::Lab);
        assert_eq!(classify_domain("Quels sont mes résultats d'analyse?"), QueryDomain::Lab);
    }

    #[test]
    fn classify_symptom_queries() {
        assert_eq!(classify_domain("I've been feeling dizzy"), QueryDomain::Symptom);
        assert_eq!(classify_domain("Is this a side effect?"), QueryDomain::Symptom);
        assert_eq!(classify_domain("J'ai mal à la tête"), QueryDomain::Symptom);
    }

    #[test]
    fn classify_allergy_queries() {
        assert_eq!(classify_domain("Am I allergic to penicillin?"), QueryDomain::Allergy);
        assert_eq!(classify_domain("Can I take ibuprofen?"), QueryDomain::Allergy);
        assert_eq!(classify_domain("Je suis allergique à quoi?"), QueryDomain::Allergy);
    }

    #[test]
    fn classify_diagnosis_queries() {
        assert_eq!(classify_domain("What conditions do I have?"), QueryDomain::Diagnosis);
        assert_eq!(classify_domain("When was I diagnosed?"), QueryDomain::Diagnosis);
    }

    #[test]
    fn classify_timeline_queries() {
        assert_eq!(classify_domain("What changed since my last visit?"), QueryDomain::Timeline);
        assert_eq!(classify_domain("Show me the progression"), QueryDomain::Timeline);
    }

    #[test]
    fn classify_general_queries() {
        assert_eq!(classify_domain("Tell me about my health"), QueryDomain::General);
        assert_eq!(classify_domain("Hello"), QueryDomain::General);
    }

    // ── Cross-domain matrix ───────────────────────────────────────

    #[test]
    fn allergy_high_for_medication_query() {
        let d = domain_relevance(ItemType::Allergy, QueryDomain::Medication);
        assert_eq!(d, 2.0, "Allergies are safety-critical for medication queries");
    }

    #[test]
    fn medication_high_for_allergy_query() {
        let d = domain_relevance(ItemType::Medication, QueryDomain::Allergy);
        assert_eq!(d, 1.8, "Medications are highly relevant for allergy queries");
    }

    #[test]
    fn self_domain_is_maximum() {
        assert_eq!(domain_relevance(ItemType::Medication, QueryDomain::Medication), 2.0);
        assert_eq!(domain_relevance(ItemType::LabResult, QueryDomain::Lab), 2.0);
        assert_eq!(domain_relevance(ItemType::Symptom, QueryDomain::Symptom), 2.0);
        assert_eq!(domain_relevance(ItemType::Diagnosis, QueryDomain::Diagnosis), 2.0);
        assert_eq!(domain_relevance(ItemType::Allergy, QueryDomain::Allergy), 2.0);
    }

    #[test]
    fn timeline_equal_weight() {
        assert_eq!(domain_relevance(ItemType::Medication, QueryDomain::Timeline), 1.0);
        assert_eq!(domain_relevance(ItemType::LabResult, QueryDomain::Timeline), 1.0);
        assert_eq!(domain_relevance(ItemType::Symptom, QueryDomain::Timeline), 1.0);
    }

    #[test]
    fn general_equal_weight() {
        assert_eq!(domain_relevance(ItemType::Medication, QueryDomain::General), 1.0);
        assert_eq!(domain_relevance(ItemType::LabResult, QueryDomain::General), 1.0);
    }

    #[test]
    fn domain_range_within_bounds() {
        let types = [
            ItemType::Medication, ItemType::LabResult, ItemType::Diagnosis,
            ItemType::Allergy, ItemType::Symptom, ItemType::VitalSign,
        ];
        let domains = [
            QueryDomain::Medication, QueryDomain::Lab, QueryDomain::Symptom,
            QueryDomain::Diagnosis, QueryDomain::Allergy, QueryDomain::Procedure,
            QueryDomain::Timeline, QueryDomain::General,
        ];
        for it in &types {
            for qd in &domains {
                let d = domain_relevance(*it, *qd);
                assert!(d >= 0.2 && d <= 2.0,
                    "D({:?}, {:?}) = {} out of range [0.2, 2.0]", it, qd, d);
            }
        }
    }
}
