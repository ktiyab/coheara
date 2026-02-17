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

/// Compute structuring confidence INDEPENDENTLY of LLM-reported confidence (SEC-01-D08).
///
/// Formula: 30% entity presence + 30% format quality + 40% OCR confidence.
/// - Entity presence: 0.30 if any entities extracted, 0.0 otherwise.
/// - Format quality: 0.30 baseline, reduced by validation warnings (each warning -0.06).
/// - OCR confidence: 0.40 * ocr_confidence.
pub fn compute_structuring_confidence(
    ocr_confidence: f32,
    entities: &ExtractedEntities,
    validation_warning_count: usize,
) -> f32 {
    // Entity presence: 30%
    let entity_component = if count_entities(entities) > 0 {
        0.30
    } else {
        0.0
    };

    // Format quality: 30% baseline, reduced by warnings
    // Each warning reduces by 0.06 (5 warnings zero it out)
    let warning_penalty = (validation_warning_count as f32 * 0.06).min(0.30);
    let format_component = 0.30 - warning_penalty;

    // OCR confidence: 40%
    let ocr_component = ocr_confidence * 0.40;

    (entity_component + format_component + ocr_component).clamp(0.0, 1.0)
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

/// Generate user-facing warnings when structuring confidence is below thresholds (K.7).
/// Returns warnings that should be appended to validation_warnings.
pub fn generate_confidence_warnings(structuring_confidence: f32) -> Vec<String> {
    let mut warnings = Vec::new();

    if structuring_confidence < structuring_thresholds::LOW {
        warnings.push(format!(
            "Low structuring confidence ({:.0}%). Results may be unreliable — manual verification strongly recommended.",
            structuring_confidence * 100.0
        ));
    } else if structuring_confidence < structuring_thresholds::MODERATE {
        warnings.push(format!(
            "Moderate structuring confidence ({:.0}%). Some extracted fields may need manual review.",
            structuring_confidence * 100.0
        ));
    }

    warnings
}

/// Apply OCR-based confidence capping to all extracted entities.
/// Caps each entity's confidence at (ocr_confidence + 0.05).
pub fn apply_confidence_caps(entities: &mut ExtractedEntities, ocr_confidence: f32) {
    for med in &mut entities.medications {
        med.confidence = adjust_entity_confidence(med.confidence, ocr_confidence);
    }
    for lab in &mut entities.lab_results {
        lab.confidence = adjust_entity_confidence(lab.confidence, ocr_confidence);
    }
    for dx in &mut entities.diagnoses {
        dx.confidence = adjust_entity_confidence(dx.confidence, ocr_confidence);
    }
    for allergy in &mut entities.allergies {
        allergy.confidence = adjust_entity_confidence(allergy.confidence, ocr_confidence);
    }
    for proc in &mut entities.procedures {
        proc.confidence = adjust_entity_confidence(proc.confidence, ocr_confidence);
    }
    for referral in &mut entities.referrals {
        referral.confidence = adjust_entity_confidence(referral.confidence, ocr_confidence);
    }
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

    // ── Independent confidence formula (SEC-01-D08) ─────────────────

    #[test]
    fn high_ocr_rich_entities_no_warnings_high_confidence() {
        // 0.30 (entities) + 0.30 (format) + 0.40*0.90 (OCR) = 0.96
        let conf = compute_structuring_confidence(0.90, &rich_entities(), 0);
        assert!(conf > 0.90, "Expected > 0.90, got {conf}");
    }

    #[test]
    fn high_ocr_no_entities_penalized() {
        // 0.00 (no entities) + 0.30 (format) + 0.40*0.90 = 0.66
        let conf = compute_structuring_confidence(0.90, &empty_entities(), 0);
        assert!(conf < 0.70, "Expected < 0.70, got {conf}");
        assert!(conf > 0.60, "Expected > 0.60, got {conf}");
    }

    #[test]
    fn low_ocr_still_bounded() {
        // 0.30 + 0.30 + 0.40*0.30 = 0.72
        let conf = compute_structuring_confidence(0.30, &rich_entities(), 0);
        assert!(conf < 0.80, "Expected < 0.80, got {conf}");
        assert!(conf > 0.60, "Expected > 0.60, got {conf}");
    }

    #[test]
    fn zero_ocr_no_entities_is_format_only() {
        // 0.00 + 0.30 + 0.00 = 0.30
        let conf = compute_structuring_confidence(0.0, &empty_entities(), 0);
        assert!((conf - 0.30).abs() < 0.01, "Expected ~0.30, got {conf}");
    }

    #[test]
    fn perfect_ocr_capped_at_one() {
        let conf = compute_structuring_confidence(1.0, &rich_entities(), 0);
        assert!(conf <= 1.0, "Expected <= 1.0, got {conf}");
    }

    #[test]
    fn warnings_reduce_format_component() {
        // 3 warnings: format = 0.30 - 3*0.06 = 0.12
        // 0.30 + 0.12 + 0.40*0.90 = 0.78
        let conf = compute_structuring_confidence(0.90, &rich_entities(), 3);
        assert!(conf < 0.85, "Expected < 0.85, got {conf}");
        assert!(conf > 0.70, "Expected > 0.70, got {conf}");
    }

    #[test]
    fn five_plus_warnings_zero_format_component() {
        // 5+ warnings: format = 0.00
        // 0.30 + 0.00 + 0.40*0.90 = 0.66
        let conf = compute_structuring_confidence(0.90, &rich_entities(), 5);
        assert!((conf - 0.66).abs() < 0.01, "Expected ~0.66, got {conf}");
    }

    #[test]
    fn worst_case_many_warnings_no_entities_low_ocr() {
        // 0.00 + 0.00 + 0.40*0.10 = 0.04
        let conf = compute_structuring_confidence(0.10, &empty_entities(), 10);
        assert!(conf < 0.10, "Expected < 0.10, got {conf}");
    }

    // ── Entity confidence capping (unchanged) ───────────────────────

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

    // ── apply_confidence_caps (J.2) ──────────────────────────────────

    #[test]
    fn apply_caps_reduces_entity_confidence_for_low_ocr() {
        let mut entities = rich_entities();
        // Set high confidence on all entities
        entities.medications[0].confidence = 0.95;
        entities.diagnoses[0].confidence = 0.90;
        entities.allergies[0].confidence = 0.88;

        // Apply with low OCR
        apply_confidence_caps(&mut entities, 0.50);

        // All should be capped at 0.55 (0.50 + 0.05)
        assert!(
            entities.medications[0].confidence <= 0.55,
            "Med confidence should be capped, got {}",
            entities.medications[0].confidence
        );
        assert!(
            entities.diagnoses[0].confidence <= 0.55,
            "Diagnosis confidence should be capped, got {}",
            entities.diagnoses[0].confidence
        );
        assert!(
            entities.allergies[0].confidence <= 0.55,
            "Allergy confidence should be capped, got {}",
            entities.allergies[0].confidence
        );
    }

    // ── K.7: Confidence warnings ─────────────────────────────────────

    #[test]
    fn low_confidence_generates_warning() {
        let warnings = generate_confidence_warnings(0.40);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Low structuring confidence"));
        assert!(warnings[0].contains("40%"));
    }

    #[test]
    fn moderate_confidence_generates_warning() {
        let warnings = generate_confidence_warnings(0.60);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Moderate structuring confidence"));
        assert!(warnings[0].contains("60%"));
    }

    #[test]
    fn high_confidence_no_warning() {
        let warnings = generate_confidence_warnings(0.85);
        assert!(warnings.is_empty());
    }

    #[test]
    fn borderline_low_threshold() {
        // Exactly at LOW (0.50) should still trigger moderate warning
        let warnings = generate_confidence_warnings(0.50);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Moderate"));
    }

    #[test]
    fn borderline_moderate_threshold() {
        // Exactly at MODERATE (0.70) should NOT trigger any warning
        let warnings = generate_confidence_warnings(0.70);
        assert!(warnings.is_empty());
    }

    #[test]
    fn apply_caps_preserves_confidence_for_high_ocr() {
        let mut entities = rich_entities();
        entities.medications[0].confidence = 0.80;

        apply_confidence_caps(&mut entities, 0.95);

        // 0.80 < 1.00 cap — unchanged
        assert!(
            (entities.medications[0].confidence - 0.80).abs() < f32::EPSILON,
            "Should be unchanged, got {}",
            entities.medications[0].confidence
        );
    }
}
