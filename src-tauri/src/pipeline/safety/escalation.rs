//! Layer 4: Escalation Check (Spec 44 [SF-03]).
//!
//! Detects when a model response is INSUFFICIENTLY urgent for known-dangerous
//! scenarios. Fires based on the QUERY (not the response) — we do not trust
//! the model for pediatric emergency triage.
//!
//! Rules are hard-coded from AAP, WHO, and French HAS guidelines.

use serde::{Deserialize, Serialize};

/// Severity of an escalation rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EscalationSeverity {
    /// Call emergency services. Red banner.
    Emergency,
    /// See a doctor urgently (same day). Orange banner.
    Urgent,
    /// Schedule appointment soon. Yellow banner.
    Advisory,
}

/// What action to take when a rule fires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscalationAction {
    /// Replace model response entirely with emergency message.
    ReplaceWithEmergency { message_key: &'static str },
    /// Prepend emergency message BEFORE model response.
    PrependWarning { message_key: &'static str },
    /// Append warning AFTER model response.
    AppendWarning { message_key: &'static str },
}

/// Result of an escalation check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationResult {
    /// Which rule fired.
    pub rule_id: &'static str,
    /// Severity level.
    pub severity: EscalationSeverity,
    /// The localized message to display.
    pub message: String,
    /// Whether to replace the model response entirely.
    pub replaces_response: bool,
}

/// A hard-coded escalation rule.
struct EscalationRule {
    /// Unique identifier for audit trail.
    id: &'static str,
    /// Condition: when should this rule fire?
    condition: EscalationCondition,
    /// Action to take.
    action: EscalationAction,
    /// Severity level.
    severity: EscalationSeverity,
}

/// Condition under which an escalation rule fires.
enum EscalationCondition {
    /// Profile age below threshold AND query contains any keyword.
    AgeBelow {
        months: u32,
        query_keywords: &'static [&'static str],
    },
    /// Query contains any keyword (any age).
    AnyAgeKeywords {
        query_keywords: &'static [&'static str],
    },
    /// Child (minor) + query contains keywords.
    ChildKeywords {
        query_keywords: &'static [&'static str],
    },
}

// ── Keyword sets ────────────────────────────────────────────

static FEVER_KEYWORDS: &[&str] = &[
    "fever", "temperature", "hot", "burning up",
    "fièvre", "température", "chaud",
    "fieber", "temperatur", "heiß",
];

static HIGH_FEVER_KEYWORDS: &[&str] = &[
    "high fever", "39", "40", "41", "102", "103", "104",
    "forte fièvre", "39°", "40°",
    "hohes fieber",
];

static BREATHING_KEYWORDS: &[&str] = &[
    "breathing", "breathe", "breath", "choking", "blue", "cyanosis", "gasping",
    "respiration", "respirer", "souffle", "étouffer", "bleu",
    "atmung", "atmen", "erstickung", "blau",
];

static SEIZURE_KEYWORDS: &[&str] = &[
    "seizure", "convulsion", "convulsing", "shaking", "fitting",
    "convulsion", "crise", "secouer",
    "krampf", "krampfanfall", "anfall",
];

static UNRESPONSIVE_KEYWORDS: &[&str] = &[
    "unresponsive", "not responding", "unconscious", "limp", "floppy", "won't wake",
    "ne répond pas", "inconscient", "mou",
    "reagiert nicht", "bewusstlos", "schlaff",
];

static RASH_FEVER_KEYWORDS: &[&str] = &[
    "rash", "spots", "rash and fever", "spots and fever", "petechiae",
    "éruption", "boutons", "éruption et fièvre",
    "ausschlag", "flecken", "ausschlag und fieber",
];

static HEAD_INJURY_KEYWORDS: &[&str] = &[
    "head injury", "hit head", "fell on head", "head bump", "concussion",
    "blessure tête", "coup à la tête", "tombé sur la tête", "commotion",
    "kopfverletzung", "kopf gestoßen", "auf den kopf gefallen", "gehirnerschütterung",
];

// ── Rule registry ───────────────────────────────────────────

/// Build the sorted rule set. Emergency rules first, then Urgent, then Advisory.
fn rules() -> Vec<EscalationRule> {
    vec![
        // PED-001: Baby < 3mo + fever → EMERGENCY REPLACE
        EscalationRule {
            id: "PED-001",
            condition: EscalationCondition::AgeBelow {
                months: 3,
                query_keywords: FEVER_KEYWORDS,
            },
            action: EscalationAction::ReplaceWithEmergency {
                message_key: "safety.escalation.ped_001",
            },
            severity: EscalationSeverity::Emergency,
        },
        // PED-003: Any age + breathing difficulty → EMERGENCY PREPEND
        EscalationRule {
            id: "PED-003",
            condition: EscalationCondition::AnyAgeKeywords {
                query_keywords: BREATHING_KEYWORDS,
            },
            action: EscalationAction::PrependWarning {
                message_key: "safety.escalation.ped_003",
            },
            severity: EscalationSeverity::Emergency,
        },
        // PED-004: Any age + seizure → EMERGENCY REPLACE
        EscalationRule {
            id: "PED-004",
            condition: EscalationCondition::AnyAgeKeywords {
                query_keywords: SEIZURE_KEYWORDS,
            },
            action: EscalationAction::ReplaceWithEmergency {
                message_key: "safety.escalation.ped_004",
            },
            severity: EscalationSeverity::Emergency,
        },
        // PED-005: Any age + unresponsive → EMERGENCY REPLACE
        EscalationRule {
            id: "PED-005",
            condition: EscalationCondition::AnyAgeKeywords {
                query_keywords: UNRESPONSIVE_KEYWORDS,
            },
            action: EscalationAction::ReplaceWithEmergency {
                message_key: "safety.escalation.ped_005",
            },
            severity: EscalationSeverity::Emergency,
        },
        // PED-002: Baby 3-6mo + high fever → URGENT PREPEND
        EscalationRule {
            id: "PED-002",
            condition: EscalationCondition::AgeBelow {
                months: 6,
                query_keywords: HIGH_FEVER_KEYWORDS,
            },
            action: EscalationAction::PrependWarning {
                message_key: "safety.escalation.ped_002",
            },
            severity: EscalationSeverity::Urgent,
        },
        // PED-006: Any child + rash + fever → URGENT PREPEND
        EscalationRule {
            id: "PED-006",
            condition: EscalationCondition::ChildKeywords {
                query_keywords: RASH_FEVER_KEYWORDS,
            },
            action: EscalationAction::PrependWarning {
                message_key: "safety.escalation.ped_006",
            },
            severity: EscalationSeverity::Urgent,
        },
        // PED-007: Any child + head injury → URGENT PREPEND
        EscalationRule {
            id: "PED-007",
            condition: EscalationCondition::ChildKeywords {
                query_keywords: HEAD_INJURY_KEYWORDS,
            },
            action: EscalationAction::PrependWarning {
                message_key: "safety.escalation.ped_007",
            },
            severity: EscalationSeverity::Urgent,
        },
    ]
}

// ── Matching logic ──────────────────────────────────────────

/// Check if any escalation rule fires for this query + age context.
///
/// Rules are checked in severity order (Emergency first). First match wins.
/// Returns `None` if no rule fires (adult queries with no emergency keywords, etc.).
pub fn check_escalation(
    query_text: &str,
    age_months: Option<u32>,
    is_minor: bool,
    lang: &str,
) -> Option<EscalationResult> {
    let query_lower = query_text.to_lowercase();

    for rule in &rules() {
        if rule.condition.matches(&query_lower, age_months, is_minor) {
            let message = escalation_message(rule.action.message_key(), lang);
            let replaces = matches!(rule.action, EscalationAction::ReplaceWithEmergency { .. });

            tracing::warn!(
                rule_id = rule.id,
                severity = ?rule.severity,
                replaces_response = replaces,
                "Safety escalation rule fired"
            );

            return Some(EscalationResult {
                rule_id: rule.id,
                severity: rule.severity,
                message,
                replaces_response: replaces,
            });
        }
    }

    None
}

// ── Condition matching ──────────────────────────────────────

impl EscalationCondition {
    fn matches(&self, query_lower: &str, age_months: Option<u32>, is_minor: bool) -> bool {
        match self {
            Self::AgeBelow { months, query_keywords } => {
                let Some(age) = age_months else { return false };
                age < *months && query_keywords.iter().any(|kw| query_lower.contains(kw))
            }
            Self::AnyAgeKeywords { query_keywords } => {
                query_keywords.iter().any(|kw| query_lower.contains(kw))
            }
            Self::ChildKeywords { query_keywords } => {
                is_minor && query_keywords.iter().any(|kw| query_lower.contains(kw))
            }
        }
    }
}

impl EscalationAction {
    fn message_key(&self) -> &'static str {
        match self {
            Self::ReplaceWithEmergency { message_key } => message_key,
            Self::PrependWarning { message_key } => message_key,
            Self::AppendWarning { message_key } => message_key,
        }
    }
}

// ── I18n messages ───────────────────────────────────────────

fn escalation_message(key: &str, lang: &str) -> String {
    match (key, lang) {
        // PED-001: Baby < 3mo + fever → EMERGENCY
        ("safety.escalation.ped_001", "fr") =>
            "**IMPORTANT : Pour les bébés de moins de 3 mois, TOUTE fièvre (38,0°C ou plus) nécessite une évaluation médicale immédiate. Veuillez contacter votre pédiatre ou vous rendre aux urgences immédiatement. N'attendez pas.**".into(),
        ("safety.escalation.ped_001", "de") =>
            "**WICHTIG: Bei Babys unter 3 Monaten erfordert JEDES Fieber (38,0°C oder höher) eine sofortige ärztliche Untersuchung. Bitte kontaktieren Sie Ihren Kinderarzt oder fahren Sie sofort in die Notaufnahme. Warten Sie nicht.**".into(),
        ("safety.escalation.ped_001", _) =>
            "**IMPORTANT: For babies under 3 months old, ANY fever (38.0\u{00b0}C / 100.4\u{00b0}F or higher) requires immediate medical evaluation. Please contact your pediatrician or go to the emergency room right away. Do not wait.**".into(),

        // PED-002: Baby 3-6mo + high fever → URGENT
        ("safety.escalation.ped_002", "fr") =>
            "**Attention : Pour les bébés de 3 à 6 mois avec une forte fièvre, une consultation médicale rapide est recommandée. Contactez votre pédiatre dans la journée.**".into(),
        ("safety.escalation.ped_002", "de") =>
            "**Achtung: Bei Babys von 3 bis 6 Monaten mit hohem Fieber wird eine zeitnahe ärztliche Beratung empfohlen. Kontaktieren Sie noch heute Ihren Kinderarzt.**".into(),
        ("safety.escalation.ped_002", _) =>
            "**Attention: For babies 3-6 months old with a high fever, prompt medical consultation is recommended. Contact your pediatrician today.**".into(),

        // PED-003: Breathing difficulty → EMERGENCY
        ("safety.escalation.ped_003", "fr") =>
            "**URGENT : Les difficultés respiratoires nécessitent une attention médicale immédiate. Si la personne a du mal à respirer, appelez les services d'urgence (15 ou 112) maintenant.**".into(),
        ("safety.escalation.ped_003", "de") =>
            "**DRINGEND: Atembeschwerden erfordern sofortige ärztliche Hilfe. Wenn die Person Schwierigkeiten beim Atmen hat, rufen Sie jetzt den Notdienst (112) an.**".into(),
        ("safety.escalation.ped_003", _) =>
            "**URGENT: Breathing difficulties require immediate medical attention. If the person is having trouble breathing, call emergency services (911) now.**".into(),

        // PED-004: Seizure → EMERGENCY
        ("safety.escalation.ped_004", "fr") =>
            "**URGENCE : Les convulsions nécessitent une attention médicale immédiate. Appelez les services d'urgence (15 ou 112). Placez la personne en position latérale de sécurité et ne mettez rien dans sa bouche.**".into(),
        ("safety.escalation.ped_004", "de") =>
            "**NOTFALL: Krampfanfälle erfordern sofortige ärztliche Hilfe. Rufen Sie den Notdienst (112) an. Legen Sie die Person in die stabile Seitenlage und stecken Sie nichts in den Mund.**".into(),
        ("safety.escalation.ped_004", _) =>
            "**EMERGENCY: Seizures require immediate medical attention. Call emergency services (911). Place the person on their side and do not put anything in their mouth.**".into(),

        // PED-005: Unresponsive → EMERGENCY
        ("safety.escalation.ped_005", "fr") =>
            "**URGENCE : Une personne qui ne répond pas nécessite une aide médicale immédiate. Appelez les services d'urgence (15 ou 112) immédiatement.**".into(),
        ("safety.escalation.ped_005", "de") =>
            "**NOTFALL: Eine nicht reagierende Person benötigt sofortige medizinische Hilfe. Rufen Sie sofort den Notdienst (112) an.**".into(),
        ("safety.escalation.ped_005", _) =>
            "**EMERGENCY: An unresponsive person requires immediate medical help. Call emergency services (911) immediately.**".into(),

        // PED-006: Child + rash + fever → URGENT
        ("safety.escalation.ped_006", "fr") =>
            "**Attention : Une éruption cutanée accompagnée de fièvre chez un enfant doit être évaluée par un médecin rapidement. Consultez votre médecin dans la journée.**".into(),
        ("safety.escalation.ped_006", "de") =>
            "**Achtung: Ein Hautausschlag mit Fieber bei einem Kind sollte zeitnah von einem Arzt untersucht werden. Konsultieren Sie noch heute Ihren Arzt.**".into(),
        ("safety.escalation.ped_006", _) =>
            "**Attention: A rash with fever in a child should be evaluated by a doctor promptly. Consult your doctor today.**".into(),

        // PED-007: Child + head injury → URGENT
        ("safety.escalation.ped_007", "fr") =>
            "**Attention : Les blessures à la tête chez les enfants doivent être évaluées par un professionnel de santé. Si l'enfant vomit, est somnolent ou semble confus, rendez-vous aux urgences.**".into(),
        ("safety.escalation.ped_007", "de") =>
            "**Achtung: Kopfverletzungen bei Kindern sollten von einem Arzt untersucht werden. Wenn das Kind erbricht, schläfrig ist oder verwirrt erscheint, fahren Sie in die Notaufnahme.**".into(),
        ("safety.escalation.ped_007", _) =>
            "**Attention: Head injuries in children should be evaluated by a healthcare professional. If the child vomits, is drowsy, or seems confused, go to the emergency room.**".into(),

        // Fallback for unknown keys
        (_, _) =>
            "**Please consult a healthcare professional about this concern.**".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── PED-001: Baby < 3mo + fever ────────────────────────────

    #[test]
    fn ped_001_fires_for_2mo_baby_with_fever() {
        let result = check_escalation("my baby has a fever", Some(2), true, "en");
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.rule_id, "PED-001");
        assert_eq!(r.severity, EscalationSeverity::Emergency);
        assert!(r.replaces_response);
    }

    #[test]
    fn ped_001_does_not_fire_for_4mo_baby() {
        let result = check_escalation("my baby has a fever", Some(4), true, "en");
        // Should NOT be PED-001 (age >= 3mo). Could be something else.
        if let Some(r) = &result {
            assert_ne!(r.rule_id, "PED-001");
        }
    }

    #[test]
    fn ped_001_fires_french_keywords() {
        let result = check_escalation("mon bébé a de la fièvre", Some(1), true, "fr");
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.rule_id, "PED-001");
        assert!(r.message.contains("pédiatre"));
    }

    #[test]
    fn ped_001_fires_german_keywords() {
        let result = check_escalation("mein baby hat fieber", Some(2), true, "de");
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.rule_id, "PED-001");
        assert!(r.message.contains("Kinderarzt"));
    }

    #[test]
    fn ped_001_boundary_3mo_does_not_fire() {
        // Exactly 3 months = NOT under 3 months
        let result = check_escalation("baby has fever", Some(3), true, "en");
        if let Some(r) = &result {
            assert_ne!(r.rule_id, "PED-001");
        }
    }

    // ── PED-002: Baby 3-6mo + high fever ───────────────────────

    #[test]
    fn ped_002_fires_for_4mo_high_fever() {
        let result = check_escalation("baby has high fever 39 degrees", Some(4), true, "en");
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.rule_id, "PED-002");
        assert_eq!(r.severity, EscalationSeverity::Urgent);
        assert!(!r.replaces_response);
    }

    #[test]
    fn ped_002_does_not_fire_for_7mo() {
        let result = check_escalation("baby has high fever 39", Some(7), true, "en");
        if let Some(r) = &result {
            assert_ne!(r.rule_id, "PED-002");
        }
    }

    // ── PED-003: Breathing difficulty ──────────────────────────

    #[test]
    fn ped_003_fires_for_breathing_any_age() {
        let result = check_escalation("my child is having trouble breathing", None, false, "en");
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.rule_id, "PED-003");
        assert_eq!(r.severity, EscalationSeverity::Emergency);
    }

    #[test]
    fn ped_003_fires_for_adult_breathing() {
        let result = check_escalation("I can't breathe properly", Some(360), false, "en");
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.rule_id, "PED-003");
    }

    #[test]
    fn ped_003_fires_french() {
        let result = check_escalation("mon enfant a du mal à respirer", None, true, "fr");
        assert!(result.is_some());
        assert_eq!(result.unwrap().rule_id, "PED-003");
    }

    // ── PED-004: Seizure ───────────────────────────────────────

    #[test]
    fn ped_004_fires_for_seizure() {
        let result = check_escalation("my child is having a seizure", None, true, "en");
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.rule_id, "PED-004");
        assert_eq!(r.severity, EscalationSeverity::Emergency);
        assert!(r.replaces_response);
    }

    #[test]
    fn ped_004_fires_for_convulsion_fr() {
        let result = check_escalation("mon enfant a une convulsion", None, true, "fr");
        assert!(result.is_some());
        assert_eq!(result.unwrap().rule_id, "PED-004");
    }

    // ── PED-005: Unresponsive ──────────────────────────────────

    #[test]
    fn ped_005_fires_for_unresponsive() {
        let result = check_escalation("my baby is unresponsive", Some(6), true, "en");
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.rule_id, "PED-005");
        assert!(r.replaces_response);
    }

    #[test]
    fn ped_005_fires_wont_wake() {
        let result = check_escalation("my child won't wake up", None, true, "en");
        assert!(result.is_some());
        assert_eq!(result.unwrap().rule_id, "PED-005");
    }

    // ── PED-006: Child + rash + fever ──────────────────────────

    #[test]
    fn ped_006_fires_for_child_rash_fever() {
        let result = check_escalation("my child has a rash and fever", Some(48), true, "en");
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.rule_id, "PED-006");
        assert_eq!(r.severity, EscalationSeverity::Urgent);
    }

    #[test]
    fn ped_006_does_not_fire_for_adult() {
        let result = check_escalation("I have a rash and fever", Some(360), false, "en");
        // Adult — ChildKeywords won't match
        if let Some(r) = &result {
            assert_ne!(r.rule_id, "PED-006");
        }
    }

    // ── PED-007: Child + head injury ───────────────────────────

    #[test]
    fn ped_007_fires_for_child_head_injury() {
        let result = check_escalation("my son hit his head and fell on head", Some(60), true, "en");
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.rule_id, "PED-007");
        assert_eq!(r.severity, EscalationSeverity::Urgent);
    }

    #[test]
    fn ped_007_does_not_fire_for_adult() {
        let result = check_escalation("I had a head injury yesterday", Some(480), false, "en");
        if let Some(r) = &result {
            assert_ne!(r.rule_id, "PED-007");
        }
    }

    // ── No escalation for safe queries ─────────────────────────

    #[test]
    fn no_escalation_for_medication_query() {
        let result = check_escalation("what is my metformin dose?", Some(480), false, "en");
        assert!(result.is_none());
    }

    #[test]
    fn no_escalation_for_appointment_query() {
        let result = check_escalation("when is my next appointment?", None, false, "en");
        assert!(result.is_none());
    }

    #[test]
    fn no_escalation_without_age_for_age_specific_rules() {
        // PED-001 requires age < 3mo — no age means rule doesn't fire
        let result = check_escalation("baby has fever", None, false, "en");
        // PED-003 (breathing) etc. are AnyAgeKeywords, so those would still fire
        // But fever alone without age context shouldn't trigger PED-001
        if let Some(r) = &result {
            assert_ne!(r.rule_id, "PED-001");
        }
    }

    // ── Priority ordering ──────────────────────────────────────

    #[test]
    fn emergency_takes_priority_over_urgent() {
        // A 2mo baby with fever AND breathing issues: PED-001 (Emergency) should fire before PED-002
        let result = check_escalation("baby has fever and trouble breathing", Some(2), true, "en");
        assert!(result.is_some());
        let r = result.unwrap();
        // PED-001 is emergency and listed first
        assert_eq!(r.severity, EscalationSeverity::Emergency);
    }
}
