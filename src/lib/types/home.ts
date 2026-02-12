export interface EntitySummary {
  medications: number;
  lab_results: number;
  diagnoses: number;
  allergies: number;
  procedures: number;
  referrals: number;
}

export interface DocumentCard {
  id: string;
  document_type: string;
  source_filename: string;
  professional_name: string | null;
  professional_specialty: string | null;
  document_date: string | null;
  imported_at: string;
  status: 'PendingReview' | 'Confirmed';
  entity_summary: EntitySummary;
}

export interface ProfileStats {
  total_documents: number;
  documents_pending_review: number;
  total_medications: number;
  total_lab_results: number;
  last_document_date: string | null;
  extraction_accuracy: number | null;
}

export interface OnboardingProgress {
  first_document_loaded: boolean;
  first_document_reviewed: boolean;
  first_question_asked: boolean;
  three_documents_loaded: boolean;
  first_symptom_recorded: boolean;
}

export interface HomeData {
  stats: ProfileStats;
  recent_documents: DocumentCard[];
  onboarding: OnboardingProgress;
}
