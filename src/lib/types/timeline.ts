/** L4-04: Timeline View frontend types. */

export type EventType =
  | 'MedicationStart'
  | 'MedicationStop'
  | 'MedicationDoseChange'
  | 'LabResult'
  | 'Symptom'
  | 'Procedure'
  | 'Appointment'
  | 'Document'
  | 'Diagnosis'
  | 'CoherenceAlert'
  | 'VitalSign';

export type EventSeverity = 'Normal' | 'Low' | 'Moderate' | 'High' | 'Critical';

export type ZoomLevel = 'Day' | 'Week' | 'Month' | 'Year';

export interface TimelineEvent {
  id: string;
  event_type: EventType;
  date: string;
  title: string;
  subtitle: string | null;
  professional_id: string | null;
  professional_name: string | null;
  document_id: string | null;
  severity: EventSeverity | null;
  metadata: EventMetadata;
}

export type EventMetadata =
  | { kind: 'Medication'; generic_name: string; brand_name: string | null; dose: string; frequency: string; status: string; reason: string | null; route: string | null; frequency_type: string | null; is_otc: boolean | null; condition: string | null; administration_instructions: string | null }
  | { kind: 'DoseChange'; generic_name: string; old_dose: string | null; new_dose: string; old_frequency: string | null; new_frequency: string | null; reason: string | null }
  | { kind: 'Lab'; test_name: string; value: number | null; value_text: string | null; unit: string | null; reference_low: number | null; reference_high: number | null; abnormal_flag: string }
  | { kind: 'Symptom'; category: string; specific: string; severity: number; body_region: string | null; still_active: boolean; duration: string | null; character: string | null; aggravating: string | null; relieving: string | null; timing_pattern: string | null; resolved_date: string | null; notes: string | null; source: string | null; related_medication_id: string | null; related_diagnosis_id: string | null }
  | { kind: 'Procedure'; name: string; facility: string | null; outcome: string | null; follow_up_required: boolean }
  | { kind: 'Appointment'; appointment_type: string; professional_specialty: string | null; pre_summary_generated: boolean | null; post_notes: string | null }
  | { kind: 'Document'; document_type: string; verified: boolean }
  | { kind: 'Diagnosis'; name: string; icd_code: string | null; status: string }
  | { kind: 'CoherenceAlert'; alert_type: string; severity: string; patient_message: string | null; entity_ids: string[]; dismissed: boolean; two_step_confirmed: boolean }
  | { kind: 'VitalSign'; vital_type: string; value_primary: number; value_secondary: number | null; unit: string; notes: string | null; source: string };

export interface TimelineCorrelation {
  source_id: string;
  target_id: string;
  correlation_type: string;
  description: string;
}

export interface TimelineFilter {
  event_types: EventType[] | null;
  professional_id: string | null;
  date_from: string | null;
  date_to: string | null;
  since_appointment_id: string | null;
  include_dismissed_alerts: boolean | null;
}

export interface TimelineData {
  events: TimelineEvent[];
  correlations: TimelineCorrelation[];
  date_range: DateRange;
  event_counts: EventCounts;
  professionals: ProfessionalSummary[];
}

export interface DateRange {
  earliest: string | null;
  latest: string | null;
}

export interface EventCounts {
  medications: number;
  lab_results: number;
  symptoms: number;
  procedures: number;
  appointments: number;
  documents: number;
  diagnoses: number;
  coherence_alerts: number;
  vital_signs: number;
}

export interface ProfessionalSummary {
  id: string;
  name: string;
  specialty: string | null;
  event_count: number;
}
