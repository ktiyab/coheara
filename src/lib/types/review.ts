/** L3-04 Review Screen — TypeScript types matching Rust backend structs. */

export interface ReviewData {
  document_id: string;
  original_file_path: string;
  original_file_type: 'Image' | 'Pdf';
  document_type: string;
  document_date: string | null;
  professional_name: string | null;
  professional_specialty: string | null;
  structured_markdown: string;
  extracted_fields: ExtractedField[];
  plausibility_warnings: PlausibilityWarning[];
  overall_confidence: number;
}

export interface ExtractedField {
  id: string;
  entity_type: EntityCategory;
  entity_index: number;
  field_name: string;
  display_label: string;
  value: string;
  confidence: number;
  is_flagged: boolean;
  source_hint: string | null;
}

export type EntityCategory =
  | 'Medication'
  | 'LabResult'
  | 'Diagnosis'
  | 'Allergy'
  | 'Procedure'
  | 'Referral'
  | 'Professional'
  | 'Date';

export interface PlausibilityWarning {
  field_id: string;
  warning_type: string;
  message: string;
  severity: 'Info' | 'Warning' | 'Critical';
}

export interface FieldCorrection {
  field_id: string;
  original_value: string;
  corrected_value: string;
}

export interface ReviewConfirmResult {
  document_id: string;
  status: 'Confirmed' | 'Corrected';
  entities_stored: EntitiesStoredSummary;
  corrections_applied: number;
  chunks_stored: number;
}

export interface EntitiesStoredSummary {
  medications: number;
  lab_results: number;
  diagnoses: number;
  allergies: number;
  procedures: number;
  referrals: number;
  instructions: number;
}

export interface ReviewRejectResult {
  document_id: string;
  reason: string | null;
}
