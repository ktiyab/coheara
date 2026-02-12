use super::types::ExtractedEntities;

/// Confidence thresholds for structuring quality
pub mod structuring_thresholds {
    /// Below this: structuring likely unreliable
    pub const LOW: f32 = 0.50;

    /// Below this: some entity fields may be wrong
    pub const MODERATE: f32 = 0.70;

    /// Above this: high confidence in extracted entities
    pub const HIGH: f32 = 0.85;
}

/// Assign an overall structuring confidence based on OCR confidence
/// and the richness of extracted entities.
pub fn compute_structuring_confidence(
    ocr_confidence: f32,
    entities: &ExtractedEntities,
) -> f32 {
    let entity_count = count_entities(entities);

    // Base confidence from OCR quality
    let base = ocr_confidence;

    // Boost if entities were actually extracted (model found structure)
    let entity_bonus = if entity_count == 0 {
        -0.10 // Penalty: no entities suggests structuring failed
    } else if entity_count <= 2 {
        0.0
    } else if entity_count <= 5 {
        0.02
    } else {
        0.05 // Rich extraction suggests good structuring
    };

    (base + entity_bonus).clamp(0.0, 1.0)
}

/// Count total entities across all categories.
fn count_entities(entities: &ExtractedEntities) -> usize {
    entities.medications.len()
        + entities.lab_results.len()
        + entities.diagnoses.len()
        + entities.allergies.len()
        + entities.procedures.len()
        + entities.referrals.len()
        + entities.instructions.len()
}

/// Adjust individual entity confidence based on OCR confidence.
/// If OCR was poor, cap entity confidence accordingly.
pub fn adjust_entity_confidence(entity_confidence: f32, ocr_confidence: f32) -> f32 {
    // Entity confidence can't exceed OCR confidence + small margin
    let cap = (ocr_confidence + 0.05).min(1.0);
    entity_confidence.min(cap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::structuring::types::*;

    fn empty_entities() -> ExtractedEntities {
        ExtractedEntities::default()
    }

    fn rich_entities() -> ExtractedEntities {
        ExtractedEntities {
            medications: vec![ExtractedMedication {
                generic_name: Some("Metformin".into()),
                brand_name: None,
                dose: "500mg".into(),
                frequency: "twice daily".into(),
                frequency_type: "scheduled".into(),
                route: "oral".into(),
                reason: None,
                instructions: vec![],
                is_compound: false,
                compound_ingredients: vec![],
                tapering_steps: vec![],
                max_daily_dose: None,
                condition: None,
                confidence: 0.90,
            }],
            diagnoses: vec![ExtractedDiagnosis {
                name: "Type 2 Diabetes".into(),
                icd_code: Some("E11".into()),
                date: None,
                status: "active".into(),
                confidence: 0.88,
            }],
            allergies: vec![ExtractedAllergy {
                allergen: "Penicillin".into(),
                reaction: Some("rash".into()),
                severity: Some("moderate".into()),
                confidence: 0.85,
            }],
            instructions: vec![ExtractedInstruction {
                text: "Follow up in 3 months".into(),
                category: "follow_up".into(),
            }],
            lab_results: vec![],
            procedures: vec![],
            referrals: vec![],
        }
    }

    #[test]
    fn high_ocr_rich_entities_high_confidence() {
        let conf = compute_structuring_confidence(0.90, &rich_entities());
        assert!(conf > 0.85, "Expected > 0.85, got {conf}");
    }

    #[test]
    fn high_ocr_no_entities_penalized() {
        let conf = compute_structuring_confidence(0.90, &empty_entities());
        assert!(conf < 0.85, "Expected < 0.85, got {conf}");
    }

    #[test]
    fn low_ocr_still_bounded() {
        let conf = compute_structuring_confidence(0.30, &rich_entities());
        assert!(conf < 0.50, "Expected < 0.50, got {conf}");
    }

    #[test]
    fn zero_ocr_clamped_to_zero() {
        let conf = compute_structuring_confidence(0.0, &empty_entities());
        assert_eq!(conf, 0.0);
    }

    #[test]
    fn perfect_ocr_capped_at_one() {
        let conf = compute_structuring_confidence(1.0, &rich_entities());
        assert!(conf <= 1.0, "Expected <= 1.0, got {conf}");
    }

    #[test]
    fn entity_confidence_capped_by_ocr() {
        let adjusted = adjust_entity_confidence(0.95, 0.50);
        assert!(adjusted <= 0.55, "Expected <= 0.55, got {adjusted}");
    }

    #[test]
    fn entity_confidence_uncapped_for_good_ocr() {
        let adjusted = adjust_entity_confidence(0.80, 0.95);
        assert!((adjusted - 0.80).abs() < f32::EPSILON);
    }

    #[test]
    fn entity_confidence_cap_does_not_exceed_one() {
        let adjusted = adjust_entity_confidence(1.0, 1.0);
        assert!(adjusted <= 1.0);
    }
}
