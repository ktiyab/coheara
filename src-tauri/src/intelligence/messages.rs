/// Message template builder for consistent, calm framing.
/// NC-07: No alarm wording. No red alerts. Calm, preparatory language.
/// NC-06: Every observation traces to source documents.
/// NC-08: Patient-reported vs professionally-documented distinguished.
pub struct MessageTemplates;

impl MessageTemplates {
    /// CONFLICT message.
    pub fn conflict(
        medication: &str,
        field: &str,
        value_a: &str,
        prescriber_a: &str,
        value_b: &str,
        prescriber_b: &str,
    ) -> String {
        let _ = field; // included for context but not in the message template
        format!(
            "Your records show {} {} from {} and {} {} from {}. \
             You may want to ask about this at your next appointment.",
            medication, value_a, prescriber_a, medication, value_b, prescriber_b,
        )
    }

    /// DUPLICATE message.
    pub fn duplicate(brand_a: &str, brand_b: &str, generic: &str) -> String {
        format!(
            "It looks like {} and {} may be the same medication ({}). \
             You might want to verify this with your pharmacist.",
            brand_a, brand_b, generic,
        )
    }

    /// GAP: diagnosis without treatment.
    pub fn gap_no_treatment(diagnosis: &str) -> String {
        format!(
            "Your records mention {} but I don't see a treatment plan for it. \
             This might be worth discussing at your next appointment.",
            diagnosis,
        )
    }

    /// GAP: medication without diagnosis.
    pub fn gap_no_diagnosis(medication: &str) -> String {
        format!(
            "Your records show {} as an active medication but I don't see \
             a documented reason for it. Your doctor can help clarify this.",
            medication,
        )
    }

    /// DRIFT message.
    pub fn drift(medication: &str, old_value: &str, new_value: &str) -> String {
        format!(
            "Your medication for {} was changed from {} to {}. \
             I don't see a note explaining the change. \
             You might want to ask why at your next visit.",
            medication, old_value, new_value,
        )
    }

    /// TEMPORAL message.
    pub fn temporal(symptom: &str, onset: &str, days: i64, event: &str) -> String {
        format!(
            "You reported {} starting {}, which was {} days after {}. \
             This might be worth mentioning to your doctor.",
            symptom, onset, days, event,
        )
    }

    /// ALLERGY message.
    pub fn allergy(allergen: &str, medication: &str, ingredient: &str) -> String {
        format!(
            "Your records note an allergy to {}. The medication {} \
             contains {} which is in the same family. \
             Please verify this with your pharmacist before taking it.",
            allergen, medication, ingredient,
        )
    }

    /// DOSE message.
    pub fn dose(dose: &str, medication: &str, range_low: &str, range_high: &str) -> String {
        format!(
            "I extracted {} for {} but the typical range is {}-{}. \
             Please double-check this value.",
            dose, medication, range_low, range_high,
        )
    }

    /// CRITICAL lab message.
    /// NC-07: "promptly" / "soon" — NEVER "immediately" or "urgently"
    pub fn critical_lab(date: &str, test: &str) -> String {
        format!(
            "Your lab report from {} flags {} as needing prompt attention. \
             Please contact your doctor or pharmacist soon.",
            date, test,
        )
    }

    /// Patient-reported data disclaimer (NC-08).
    pub fn patient_reported_note(symptom: &str) -> String {
        format!(
            "Note: \"{}\" is based on your own report, not a clinical document.",
            symptom,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- NC-07 calm language compliance (T-28) ---

    #[test]
    fn messages_never_contain_alarm_words() {
        let alarm_words = [
            "immediately",
            "urgently",
            "emergency",
            "danger",
            "warning",
        ];

        let messages = vec![
            MessageTemplates::conflict("Metformin", "dose", "500mg", "Dr. A", "1000mg", "Dr. B"),
            MessageTemplates::duplicate("Glucophage", "Metformin", "metformin"),
            MessageTemplates::gap_no_treatment("Type 2 Diabetes"),
            MessageTemplates::gap_no_diagnosis("Metformin"),
            MessageTemplates::drift("Metformin", "500mg", "1000mg"),
            MessageTemplates::temporal("headache", "2026-01-15", 3, "starting Lisinopril"),
            MessageTemplates::allergy("penicillin", "Amoxicillin", "amoxicillin"),
            MessageTemplates::dose("5000mg", "Metformin", "500mg", "2000mg"),
            MessageTemplates::critical_lab("2026-01-15", "Potassium"),
        ];

        for message in &messages {
            let lower = message.to_lowercase();
            for word in &alarm_words {
                assert!(
                    !lower.contains(word),
                    "Message contains alarm word '{}': {}",
                    word,
                    message,
                );
            }
        }
    }

    #[test]
    fn conflict_message_contains_both_prescribers() {
        let msg = MessageTemplates::conflict(
            "Metformin",
            "dose",
            "500mg",
            "Dr. Chen",
            "1000mg",
            "Dr. Moreau",
        );
        assert!(msg.contains("Dr. Chen"));
        assert!(msg.contains("Dr. Moreau"));
        assert!(msg.contains("500mg"));
        assert!(msg.contains("1000mg"));
    }

    #[test]
    fn duplicate_message_contains_generic() {
        let msg = MessageTemplates::duplicate("Glucophage", "Metformin", "metformin");
        assert!(msg.contains("Glucophage"));
        assert!(msg.contains("Metformin"));
        assert!(msg.contains("metformin"));
    }

    #[test]
    fn critical_lab_message_uses_calm_language() {
        let msg = MessageTemplates::critical_lab("2026-01-15", "Potassium");
        assert!(msg.contains("prompt attention"));
        assert!(msg.contains("soon"));
        assert!(!msg.contains("immediately"));
        assert!(!msg.contains("urgently"));
    }
}
