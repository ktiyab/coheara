/** E2E-F04: Document detail types. */

import type { DocumentLifecycleStatus } from './home';

// BTL-10 UX: Entity connection types (GAP-01)
export type EntityType = 'Medication' | 'LabResult' | 'Diagnosis' | 'Allergy' | 'Procedure' | 'Referral';
export type RelationshipType = 'PrescribedFor' | 'EvidencesFor' | 'MonitorsFor' | 'ContraindicatedBy' | 'FollowUpTo' | 'ReplacedBy';

export interface EntityConnection {
  id: string;
  source_type: EntityType;
  source_id: string;
  target_type: EntityType;
  target_id: string;
  relationship_type: RelationshipType;
  confidence: number;
  document_id: string;
  created_at: string;
}

// BTL-10 UX: Processing log types (GAP-04)
export interface ProcessingLogEntry {
  id: string;
  document_id: string;
  model_name: string;
  model_variant: string | null;
  processing_stage: 'Extraction' | 'Structuring';
  started_at: string;
  completed_at: string | null;
  success: boolean;
  error_message: string | null;
}

export interface DocumentDetail {
  id: string;
  document_type: string;
  title: string;
  source_filename: string;
  professional_name: string | null;
  professional_specialty: string | null;
  document_date: string | null;
  imported_at: string;
  status: DocumentLifecycleStatus;
  error_message: string | null;
  ocr_confidence: number | null;
  page_count: number | null;
  notes: string | null;
  medications: MedicationEntry[];
  lab_results: LabResultEntry[];
  diagnoses: DiagnosisEntry[];
  allergies: AllergyEntry[];
  procedures: ProcedureEntry[];
  referrals: ReferralEntry[];
}

export interface MedicationEntry {
  id: string;
  generic_name: string;
  brand_name: string | null;
  dose: string;
  frequency: string;
  route: string;
  status: string;
  start_date: string | null;
  end_date: string | null;
}

export interface LabResultEntry {
  id: string;
  test_name: string;
  value: number | null;
  value_text: string | null;
  unit: string | null;
  reference_range_low: number | null;
  reference_range_high: number | null;
  abnormal_flag: string;
  collection_date: string;
}

export interface DiagnosisEntry {
  id: string;
  name: string;
  icd_code: string | null;
  date_diagnosed: string | null;
  status: string;
}

export interface AllergyEntry {
  id: string;
  allergen: string;
  reaction: string | null;
  severity: string;
}

export interface ProcedureEntry {
  id: string;
  name: string;
  date: string | null;
  outcome: string | null;
  follow_up_required: boolean;
}

export interface ReferralEntry {
  id: string;
  reason: string | null;
  date: string;
  status: string;
}
