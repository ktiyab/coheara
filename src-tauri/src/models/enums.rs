use crate::db::DatabaseError;
use serde::{Deserialize, Serialize};

/// Macro to generate enum with as_str + std::str::FromStr pattern
macro_rules! str_enum {
    ($name:ident { $($variant:ident => $s:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$variant => $s),+
                }
            }
        }

        impl std::str::FromStr for $name {
            type Err = DatabaseError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($s => Ok(Self::$variant)),+,
                    _ => Err(DatabaseError::InvalidEnum {
                        field: stringify!($name).into(),
                        value: s.into(),
                    }),
                }
            }
        }
    };
}

str_enum!(DocumentType {
    Prescription => "prescription",
    LabResult => "lab_result",
    ClinicalNote => "clinical_note",
    DischargeSummary => "discharge_summary",
    RadiologyReport => "radiology_report",
    PharmacyRecord => "pharmacy_record",
    Other => "other",
});

str_enum!(FrequencyType {
    Scheduled => "scheduled",
    AsNeeded => "as_needed",
    Tapering => "tapering",
});

str_enum!(MedicationStatus {
    Active => "active",
    Stopped => "stopped",
    Paused => "paused",
});

str_enum!(DoseType {
    Fixed => "fixed",
    SlidingScale => "sliding_scale",
    WeightBased => "weight_based",
    Variable => "variable",
});

str_enum!(AbnormalFlag {
    Normal => "normal",
    Low => "low",
    High => "high",
    CriticalLow => "critical_low",
    CriticalHigh => "critical_high",
});

str_enum!(DiagnosisStatus {
    Active => "active",
    Resolved => "resolved",
    Monitoring => "monitoring",
});

str_enum!(AllergySeverity {
    Mild => "mild",
    Moderate => "moderate",
    Severe => "severe",
    LifeThreatening => "life_threatening",
});

str_enum!(AllergySource {
    DocumentExtracted => "document_extracted",
    PatientReported => "patient_reported",
});

str_enum!(SymptomSource {
    PatientReported => "patient_reported",
    GuidedCheckin => "guided_checkin",
    FreeText => "free_text",
});

str_enum!(AppointmentType {
    Upcoming => "upcoming",
    Completed => "completed",
});

str_enum!(ReferralStatus {
    Pending => "pending",
    Scheduled => "scheduled",
    Completed => "completed",
    Cancelled => "cancelled",
});

str_enum!(AlertType {
    Conflict => "conflict",
    Gap => "gap",
    Drift => "drift",
    Ambiguity => "ambiguity",
    Duplicate => "duplicate",
    Allergy => "allergy",
    Dose => "dose",
    Critical => "critical",
    Temporal => "temporal",
});

str_enum!(DismissedBy {
    Patient => "patient",
    ProfessionalFeedback => "professional_feedback",
});

str_enum!(MessageRole {
    Patient => "patient",
    Coheara => "coheara",
});

str_enum!(MessageFeedback {
    Helpful => "helpful",
    NotHelpful => "not_helpful",
});

str_enum!(AliasSource {
    Bundled => "bundled",
    UserAdded => "user_added",
});

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn document_type_round_trip() {
        for (variant, s) in [
            (DocumentType::Prescription, "prescription"),
            (DocumentType::LabResult, "lab_result"),
            (DocumentType::ClinicalNote, "clinical_note"),
            (DocumentType::DischargeSummary, "discharge_summary"),
            (DocumentType::RadiologyReport, "radiology_report"),
            (DocumentType::PharmacyRecord, "pharmacy_record"),
            (DocumentType::Other, "other"),
        ] {
            assert_eq!(variant.as_str(), s);
            assert_eq!(DocumentType::from_str(s).unwrap(), variant);
        }
    }

    #[test]
    fn medication_status_round_trip() {
        for (variant, s) in [
            (MedicationStatus::Active, "active"),
            (MedicationStatus::Stopped, "stopped"),
            (MedicationStatus::Paused, "paused"),
        ] {
            assert_eq!(variant.as_str(), s);
            assert_eq!(MedicationStatus::from_str(s).unwrap(), variant);
        }
    }

    #[test]
    fn abnormal_flag_round_trip() {
        for (variant, s) in [
            (AbnormalFlag::Normal, "normal"),
            (AbnormalFlag::Low, "low"),
            (AbnormalFlag::High, "high"),
            (AbnormalFlag::CriticalLow, "critical_low"),
            (AbnormalFlag::CriticalHigh, "critical_high"),
        ] {
            assert_eq!(variant.as_str(), s);
            assert_eq!(AbnormalFlag::from_str(s).unwrap(), variant);
        }
    }

    #[test]
    fn allergy_severity_round_trip() {
        for (variant, s) in [
            (AllergySeverity::Mild, "mild"),
            (AllergySeverity::Moderate, "moderate"),
            (AllergySeverity::Severe, "severe"),
            (AllergySeverity::LifeThreatening, "life_threatening"),
        ] {
            assert_eq!(variant.as_str(), s);
            assert_eq!(AllergySeverity::from_str(s).unwrap(), variant);
        }
    }

    #[test]
    fn invalid_enum_returns_error() {
        assert!(DocumentType::from_str("invalid").is_err());
        assert!(MedicationStatus::from_str("unknown").is_err());
        assert!(AbnormalFlag::from_str("").is_err());
    }
}
