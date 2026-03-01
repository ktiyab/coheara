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

/** An entity groups related fields from the same extracted item. */
export interface ReviewEntity {
  category: EntityCategory;
  entityIndex: number;
  fields: ExtractedField[];
  /** Minimum confidence across all fields in this entity. */
  confidence: number;
  /** True if any field is flagged (confidence < threshold). */
  isFlagged: boolean;
}

/** Visible entity sections per document type. */
export const SECTIONS_BY_DOC_TYPE: Record<string, EntityCategory[]> = {
  'Lab Report': ['LabResult', 'Professional', 'Date'],
  'Prescription': ['Medication', 'Professional', 'Date'],
  'Clinical Note': [
    'Diagnosis', 'Medication', 'LabResult', 'Allergy',
    'Procedure', 'Referral', 'Professional', 'Date',
  ],
  'Discharge Summary': [
    'Diagnosis', 'Medication', 'LabResult', 'Procedure',
    'Referral', 'Professional', 'Date',
  ],
  'Radiology Report': ['Diagnosis', 'Procedure', 'Professional', 'Date'],
  'Pharmacy Record': ['Medication', 'Professional', 'Date'],
  'Other': [
    'Medication', 'LabResult', 'Diagnosis', 'Allergy',
    'Procedure', 'Referral', 'Professional', 'Date',
  ],
};

/** Group flat fields into entities. Client-side only — no backend changes. */
export function groupFieldsIntoEntities(
  fields: ExtractedField[],
  documentType: string,
): ReviewEntity[] {
  const allowedCategories = SECTIONS_BY_DOC_TYPE[documentType]
    ?? SECTIONS_BY_DOC_TYPE['Other'];

  const entityMap = new Map<string, ReviewEntity>();
  for (const field of fields) {
    if (!allowedCategories.includes(field.entity_type)) continue;
    const key = `${field.entity_type}:${field.entity_index}`;
    if (!entityMap.has(key)) {
      entityMap.set(key, {
        category: field.entity_type,
        entityIndex: field.entity_index,
        fields: [],
        confidence: 1.0,
        isFlagged: false,
      });
    }
    const entity = entityMap.get(key)!;
    entity.fields.push(field);
    entity.confidence = Math.min(entity.confidence, field.confidence);
    if (field.is_flagged) entity.isFlagged = true;
  }

  return Array.from(entityMap.values());
}
