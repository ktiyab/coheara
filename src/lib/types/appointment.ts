// L4-02: Appointment Prep — TypeScript interfaces matching Rust backend types.

export interface AppointmentRequest {
  professional_id: string | null;
  new_professional: NewProfessional | null;
  date: string; // YYYY-MM-DD
}

export interface NewProfessional {
  name: string;
  specialty: string;
  institution: string | null;
}

export interface ProfessionalInfo {
  id: string;
  name: string;
  specialty: string | null;
  institution: string | null;
  last_seen_date: string | null;
}

export interface AppointmentPrep {
  appointment_id: string;
  professional_name: string;
  professional_specialty: string;
  appointment_date: string;
  patient_copy: PatientCopy;
  professional_copy: ProfessionalCopy;
  generated_at: string;
}

export interface PatientCopy {
  title: string;
  priority_items: PrepItem[];
  questions: PrepQuestion[];
  symptoms_to_mention: SymptomMention[];
  medication_changes: MedicationChange[];
  reminder: string;
}

export interface PrepItem {
  text: string;
  source: string;
  priority: string;
}

export interface PrepQuestion {
  question: string;
  context: string;
  relevance_score: number;
}

export interface SymptomMention {
  description: string;
  severity: number;
  onset_date: string;
  still_active: boolean;
}

export interface MedicationChange {
  description: string;
  change_type: string;
  date: string;
}

export interface ProfessionalCopy {
  header: ProfessionalHeader;
  current_medications: MedicationSummary[];
  changes_since_last_visit: ChangeSummary[];
  lab_results: LabSummary[];
  patient_reported_symptoms: SymptomSummary[];
  observations_for_discussion: ObservationSummary[];
  source_documents: DocumentReference[];
  disclaimer: string;
}

export interface ProfessionalHeader {
  title: string;
  date: string;
  professional: string;
  disclaimer: string;
}

export interface MedicationSummary {
  name: string;
  dose: string;
  frequency: string;
  prescriber: string;
  start_date: string;
  is_recent_change: boolean;
}

export interface ChangeSummary {
  description: string;
  date: string;
  change_type: string;
}

export interface LabSummary {
  test_name: string;
  value: string;
  unit: string;
  reference_range: string;
  abnormal_flag: string;
  date: string;
}

export interface SymptomSummary {
  description: string;
  severity: number;
  onset_date: string;
  duration: string | null;
}

export interface ObservationSummary {
  observation: string;
  severity: string;
  source: string;
}

export interface DocumentReference {
  document_type: string;
  date: string;
  professional: string;
}

export interface PostAppointmentNotes {
  appointment_id: string;
  doctor_said: string;
  changes_made: string;
  follow_up: string | null;
  general_notes: string | null;
}

export interface StoredAppointment {
  id: string;
  professional_name: string;
  professional_specialty: string;
  date: string;
  appointment_type: string;
  prep_generated: boolean;
  has_post_notes: boolean;
}

/** SEC-02-G06: PDF export result with PHI safety warning. */
export interface PdfExportResult {
  paths: string[];
  phi_warning: string;
}

export const SPECIALTIES = [
  'GP',
  'Cardiologist',
  'Neurologist',
  'Dermatologist',
  'Endocrinologist',
  'Gastroenterologist',
  'Oncologist',
  'Pharmacist',
  'Nurse',
  'Specialist',
  'Other',
] as const;
