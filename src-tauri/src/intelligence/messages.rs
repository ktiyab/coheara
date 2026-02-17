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

    /// DAILY DOSE ACCUMULATION message.
    pub fn daily_dose(
        medication: &str,
        daily_total: &str,
        max_daily: &str,
    ) -> String {
        format!(
            "Based on your {} schedule, the daily total comes to {} but the \
             recommended maximum is {}. You may want to confirm this with your \
             pharmacist or doctor.",
            medication, daily_total, max_daily,
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

/// I18N-07/08/09: Localized message template builder.
/// Dispatches to EN/FR/DE based on language code.
/// FR: formal "vous", DE: formal "Sie".
pub struct MessageTemplatesI18n;

impl MessageTemplatesI18n {
    pub fn conflict(
        lang: &str,
        medication: &str,
        field: &str,
        value_a: &str,
        prescriber_a: &str,
        value_b: &str,
        prescriber_b: &str,
    ) -> String {
        match lang {
            "fr" => {
                let _ = field;
                format!(
                    "Vos dossiers indiquent {} {} prescrit par {} et {} {} prescrit par {}. \
                     Vous pourriez en parler lors de votre prochain rendez-vous.",
                    medication, value_a, prescriber_a, medication, value_b, prescriber_b,
                )
            }
            "de" => {
                let _ = field;
                format!(
                    "Ihre Unterlagen zeigen {} {} von {} und {} {} von {}. \
                     Sie könnten dies bei Ihrem nächsten Termin ansprechen.",
                    medication, value_a, prescriber_a, medication, value_b, prescriber_b,
                )
            }
            _ => MessageTemplates::conflict(medication, field, value_a, prescriber_a, value_b, prescriber_b),
        }
    }

    pub fn duplicate(lang: &str, brand_a: &str, brand_b: &str, generic: &str) -> String {
        match lang {
            "fr" => format!(
                "Il semble que {} et {} soient le même médicament ({}). \
                 Vous pourriez vérifier cela auprès de votre pharmacien.",
                brand_a, brand_b, generic,
            ),
            "de" => format!(
                "Es scheint, dass {} und {} dasselbe Medikament sind ({}). \
                 Sie könnten dies bei Ihrem Apotheker überprüfen.",
                brand_a, brand_b, generic,
            ),
            _ => MessageTemplates::duplicate(brand_a, brand_b, generic),
        }
    }

    pub fn gap_no_treatment(lang: &str, diagnosis: &str) -> String {
        match lang {
            "fr" => format!(
                "Vos dossiers mentionnent {} mais je ne vois pas de plan de traitement correspondant. \
                 Cela pourrait valoir la peine d'en discuter lors de votre prochain rendez-vous.",
                diagnosis,
            ),
            "de" => format!(
                "Ihre Unterlagen erwähnen {} aber ich sehe keinen Behandlungsplan dafür. \
                 Dies könnte bei Ihrem nächsten Termin besprochen werden.",
                diagnosis,
            ),
            _ => MessageTemplates::gap_no_treatment(diagnosis),
        }
    }

    pub fn gap_no_diagnosis(lang: &str, medication: &str) -> String {
        match lang {
            "fr" => format!(
                "Vos dossiers indiquent {} comme médicament actif mais je ne vois pas \
                 de raison documentée. Votre médecin peut vous aider à clarifier cela.",
                medication,
            ),
            "de" => format!(
                "Ihre Unterlagen zeigen {} als aktives Medikament, aber ich sehe keinen \
                 dokumentierten Grund dafür. Ihr Arzt kann Ihnen dies erklären.",
                medication,
            ),
            _ => MessageTemplates::gap_no_diagnosis(medication),
        }
    }

    pub fn drift(lang: &str, medication: &str, old_value: &str, new_value: &str) -> String {
        match lang {
            "fr" => format!(
                "Votre médicament {} a été modifié de {} à {}. \
                 Je ne vois pas de note expliquant ce changement. \
                 Vous pourriez demander pourquoi lors de votre prochaine visite.",
                medication, old_value, new_value,
            ),
            "de" => format!(
                "Ihr Medikament {} wurde von {} auf {} geändert. \
                 Ich sehe keine Notiz, die die Änderung erklärt. \
                 Sie könnten bei Ihrem nächsten Besuch danach fragen.",
                medication, old_value, new_value,
            ),
            _ => MessageTemplates::drift(medication, old_value, new_value),
        }
    }

    pub fn allergy(lang: &str, allergen: &str, medication: &str, ingredient: &str) -> String {
        match lang {
            "fr" => format!(
                "Vos dossiers notent une allergie à {}. Le médicament {} \
                 contient {} qui appartient à la même famille. \
                 Veuillez vérifier cela auprès de votre pharmacien avant de le prendre.",
                allergen, medication, ingredient,
            ),
            "de" => format!(
                "Ihre Unterlagen vermerken eine Allergie gegen {}. Das Medikament {} \
                 enthält {} aus derselben Wirkstoffgruppe. \
                 Bitte überprüfen Sie dies mit Ihrem Apotheker, bevor Sie es einnehmen.",
                allergen, medication, ingredient,
            ),
            _ => MessageTemplates::allergy(allergen, medication, ingredient),
        }
    }

    pub fn dose(lang: &str, dose: &str, medication: &str, range_low: &str, range_high: &str) -> String {
        match lang {
            "fr" => format!(
                "J'ai extrait {} pour {} mais la plage habituelle est {}-{}. \
                 Veuillez vérifier cette valeur.",
                dose, medication, range_low, range_high,
            ),
            "de" => format!(
                "Ich habe {} für {} entnommen, aber der übliche Bereich liegt bei {}-{}. \
                 Bitte überprüfen Sie diesen Wert.",
                dose, medication, range_low, range_high,
            ),
            _ => MessageTemplates::dose(dose, medication, range_low, range_high),
        }
    }

    pub fn critical_lab(lang: &str, date: &str, test: &str) -> String {
        match lang {
            "fr" => format!(
                "Votre bilan biologique du {} signale {} comme nécessitant une attention rapide. \
                 Veuillez contacter votre médecin ou pharmacien prochainement.",
                date, test,
            ),
            "de" => format!(
                "Ihr Laborbericht vom {} markiert {} als zeitnah klärungsbedürftig. \
                 Bitte kontaktieren Sie Ihren Arzt oder Apotheker zeitnah.",
                date, test,
            ),
            _ => MessageTemplates::critical_lab(date, test),
        }
    }

    pub fn patient_reported_note(lang: &str, symptom: &str) -> String {
        match lang {
            "fr" => format!(
                "Note : \"{}\" est basé sur votre propre signalement, et non sur un document clinique.",
                symptom,
            ),
            "de" => format!(
                "Hinweis: \"{}\" basiert auf Ihrer eigenen Angabe, nicht auf einem klinischen Dokument.",
                symptom,
            ),
            _ => MessageTemplates::patient_reported_note(symptom),
        }
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
            MessageTemplates::daily_dose("Metformin", "3000mg", "2550mg"),
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

    // =================================================================
    // I18N-07/08/09: Translated coherence messages
    // =================================================================

    #[test]
    fn i18n_fr_messages_use_formal_vous() {
        let messages = vec![
            MessageTemplatesI18n::conflict("fr", "Metformine", "dose", "500mg", "Dr. A", "1000mg", "Dr. B"),
            MessageTemplatesI18n::duplicate("fr", "Glucophage", "Metformine", "metformine"),
            MessageTemplatesI18n::gap_no_treatment("fr", "Diabète de type 2"),
            MessageTemplatesI18n::gap_no_diagnosis("fr", "Metformine"),
            MessageTemplatesI18n::drift("fr", "Metformine", "500mg", "1000mg"),
            MessageTemplatesI18n::allergy("fr", "pénicilline", "Amoxicilline", "amoxicilline"),
            MessageTemplatesI18n::dose("fr", "5000mg", "Metformine", "500mg", "2000mg"),
            MessageTemplatesI18n::critical_lab("fr", "15/01/2026", "Potassium"),
        ];
        for msg in &messages {
            let lower = msg.to_lowercase();
            assert!(
                lower.contains("vous") || lower.contains("votre") || lower.contains("vos") || lower.contains("veuillez"),
                "French message missing formal address: {msg}",
            );
        }
    }

    #[test]
    fn i18n_de_messages_use_formal_sie() {
        let messages = vec![
            MessageTemplatesI18n::conflict("de", "Metformin", "dose", "500mg", "Dr. A", "1000mg", "Dr. B"),
            MessageTemplatesI18n::duplicate("de", "Glucophage", "Metformin", "Metformin"),
            MessageTemplatesI18n::gap_no_treatment("de", "Typ-2-Diabetes"),
            MessageTemplatesI18n::gap_no_diagnosis("de", "Metformin"),
            MessageTemplatesI18n::drift("de", "Metformin", "500mg", "1000mg"),
            MessageTemplatesI18n::allergy("de", "Penicillin", "Amoxicillin", "Amoxicillin"),
            MessageTemplatesI18n::dose("de", "5000mg", "Metformin", "500mg", "2000mg"),
            MessageTemplatesI18n::critical_lab("de", "15.01.2026", "Kalium"),
        ];
        for msg in &messages {
            assert!(
                msg.contains("Sie") || msg.contains("Ihre") || msg.contains("Ihrem") || msg.contains("Ihren"),
                "German message missing formal address: {msg}",
            );
        }
    }

    #[test]
    fn i18n_fr_messages_no_alarm_words() {
        let fr_alarm_words = ["immédiatement", "urgence", "danger", "alerte"];
        let messages = vec![
            MessageTemplatesI18n::conflict("fr", "M", "d", "a", "Dr.A", "b", "Dr.B"),
            MessageTemplatesI18n::critical_lab("fr", "2026-01-15", "Potassium"),
            MessageTemplatesI18n::allergy("fr", "pénicilline", "Amoxicilline", "amoxicilline"),
        ];
        for msg in &messages {
            let lower = msg.to_lowercase();
            for word in &fr_alarm_words {
                assert!(!lower.contains(word), "FR message contains alarm '{word}': {msg}");
            }
        }
    }

    #[test]
    fn i18n_de_messages_no_alarm_words() {
        let de_alarm_words = ["sofort", "notfall", "gefahr", "warnung"];
        let messages = vec![
            MessageTemplatesI18n::conflict("de", "M", "d", "a", "Dr.A", "b", "Dr.B"),
            MessageTemplatesI18n::critical_lab("de", "2026-01-15", "Kalium"),
            MessageTemplatesI18n::allergy("de", "Penicillin", "Amoxicillin", "Amoxicillin"),
        ];
        for msg in &messages {
            let lower = msg.to_lowercase();
            for word in &de_alarm_words {
                assert!(!lower.contains(word), "DE message contains alarm '{word}': {msg}");
            }
        }
    }

    #[test]
    fn i18n_en_defaults_match_original() {
        let en = MessageTemplatesI18n::conflict("en", "Metformin", "dose", "500mg", "Dr. A", "1000mg", "Dr. B");
        let orig = MessageTemplates::conflict("Metformin", "dose", "500mg", "Dr. A", "1000mg", "Dr. B");
        assert_eq!(en, orig);
    }

    #[test]
    fn i18n_patient_reported_fr() {
        let msg = MessageTemplatesI18n::patient_reported_note("fr", "maux de tête");
        assert!(msg.contains("signalement"));
        assert!(msg.contains("maux de tête"));
    }

    #[test]
    fn i18n_patient_reported_de() {
        let msg = MessageTemplatesI18n::patient_reported_note("de", "Kopfschmerzen");
        assert!(msg.contains("Angabe"));
        assert!(msg.contains("Kopfschmerzen"));
    }
}
