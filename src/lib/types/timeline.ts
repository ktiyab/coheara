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
  | 'Diagnosis';

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
  | { kind: 'Medication'; generic_name: string; brand_name: string | null; dose: string; frequency: string; status: string; reason: string | null }
  | { kind: 'DoseChange'; generic_name: string; old_dose: string | null; new_dose: string; old_frequency: string | null; new_frequency: string | null; reason: string | null }
  | { kind: 'Lab'; test_name: string; value: number | null; value_text: string | null; unit: string | null; reference_low: number | null; reference_high: number | null; abnormal_flag: string }
  | { kind: 'Symptom'; category: string; specific: string; severity: number; body_region: string | null; still_active: boolean }
  | { kind: 'Procedure'; name: string; facility: string | null; outcome: string | null; follow_up_required: boolean }
  | { kind: 'Appointment'; appointment_type: string; professional_specialty: string | null }
  | { kind: 'Document'; document_type: string; verified: boolean }
  | { kind: 'Diagnosis'; name: string; icd_code: string | null; status: string };

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
}

export interface ProfessionalSummary {
  id: string;
  name: string;
  specialty: string | null;
  event_count: number;
}
