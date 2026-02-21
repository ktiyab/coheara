use serde::{Deserialize, Serialize};

/// A single event on the timeline — unified across all entity tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: String,
    pub event_type: EventType,
    pub date: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub professional_id: Option<String>,
    pub professional_name: Option<String>,
    pub document_id: Option<String>,
    pub severity: Option<EventSeverity>,
    pub metadata: EventMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EventType {
    MedicationStart,
    MedicationStop,
    MedicationDoseChange,
    LabResult,
    Symptom,
    Procedure,
    Appointment,
    Document,
    Diagnosis,
    CoherenceAlert,
    VitalSign,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventSeverity {
    Normal,
    Low,
    Moderate,
    High,
    Critical,
}

/// Type-specific metadata carried by each event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum EventMetadata {
    Medication {
        generic_name: String,
        brand_name: Option<String>,
        dose: String,
        frequency: String,
        status: String,
        reason: Option<String>,
        route: Option<String>,
        frequency_type: Option<String>,
        is_otc: Option<bool>,
        condition: Option<String>,
        administration_instructions: Option<String>,
    },
    DoseChange {
        generic_name: String,
        old_dose: Option<String>,
        new_dose: String,
        old_frequency: Option<String>,
        new_frequency: Option<String>,
        reason: Option<String>,
    },
    Lab {
        test_name: String,
        value: Option<f64>,
        value_text: Option<String>,
        unit: Option<String>,
        reference_low: Option<f64>,
        reference_high: Option<f64>,
        abnormal_flag: String,
    },
    Symptom {
        category: String,
        specific: String,
        severity: u8,
        body_region: Option<String>,
        still_active: bool,
        duration: Option<String>,
        character: Option<String>,
        aggravating: Option<String>,
        relieving: Option<String>,
        timing_pattern: Option<String>,
        resolved_date: Option<String>,
        notes: Option<String>,
        source: Option<String>,
        related_medication_id: Option<String>,
        related_diagnosis_id: Option<String>,
    },
    Procedure {
        name: String,
        facility: Option<String>,
        outcome: Option<String>,
        follow_up_required: bool,
    },
    Appointment {
        appointment_type: String,
        professional_specialty: Option<String>,
        pre_summary_generated: Option<bool>,
        post_notes: Option<String>,
    },
    Document {
        document_type: String,
        verified: bool,
    },
    Diagnosis {
        name: String,
        icd_code: Option<String>,
        status: String,
    },
    CoherenceAlert {
        alert_type: String,
        severity: String,
        patient_message: Option<String>,
        entity_ids: Vec<String>,
        dismissed: bool,
        two_step_confirmed: bool,
    },
    VitalSign {
        vital_type: String,
        value_primary: f64,
        value_secondary: Option<f64>,
        unit: String,
        notes: Option<String>,
        source: String,
    },
}

/// A correlation between two timeline events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineCorrelation {
    pub source_id: String,
    pub target_id: String,
    pub correlation_type: CorrelationType,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CorrelationType {
    SymptomAfterMedicationChange,
    SymptomAfterMedicationStart,
    SymptomResolvedAfterMedicationStop,
    LabAfterMedicationChange,
    ExplicitLink,
    SymptomLinkedToDiagnosis,
}

/// Filter parameters sent from frontend.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimelineFilter {
    pub event_types: Option<Vec<EventType>>,
    pub professional_id: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub since_appointment_id: Option<String>,
    pub include_dismissed_alerts: Option<bool>,
}

/// Complete timeline data — single response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineData {
    pub events: Vec<TimelineEvent>,
    pub correlations: Vec<TimelineCorrelation>,
    pub date_range: DateRange,
    pub event_counts: EventCounts,
    pub professionals: Vec<ProfessionalSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub earliest: Option<String>,
    pub latest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventCounts {
    pub medications: u32,
    pub lab_results: u32,
    pub symptoms: u32,
    pub procedures: u32,
    pub appointments: u32,
    pub documents: u32,
    pub diagnoses: u32,
    pub coherence_alerts: u32,
    pub vital_signs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfessionalSummary {
    pub id: String,
    pub name: String,
    pub specialty: Option<String>,
    pub event_count: u32,
}
