/** E2E-B01: Document import types. */

/** Result of a single document import. */
export interface ImportResult {
  document_id: string;
  original_filename: string;
  format: FormatDetection;
  staged_path: string;
  duplicate_of: string | null;
  status: ImportStatus;
}

/** File format detection result from L1-01. */
export interface FormatDetection {
  mime_type: string;
  category: string;
  file_size_bytes: number;
}

/** Import status enum matching Rust ImportStatus. */
export type ImportStatus =
  | 'Staged'
  | 'Duplicate'
  | 'Unsupported'
  | 'TooLarge'
  | 'CorruptedFile';

/** Progress event emitted during single file import. */
export interface ImportProgressEvent {
  stage: string;
  file_name: string;
  document_id: string | null;
  error: string | null;
}

/** Batch progress event emitted during multi-file import. */
export interface ImportBatchProgressEvent {
  current: number;
  total: number;
  file_name: string;
}

// ---------------------------------------------------------------------------
// E2E-B02: Document Processing Types
// ---------------------------------------------------------------------------

/** Full processing outcome (import → extract → structure). */
export interface ProcessingOutcome {
  document_id: string;
  original_filename: string;
  import_status: ImportStatus;
  extraction: ExtractionSummary | null;
  structuring: StructuringSummary | null;
}

/** Summary of the extraction stage. */
export interface ExtractionSummary {
  method: string;
  confidence: number;
  page_count: number;
  text_length: number;
}

/** Summary of the structuring stage. */
export interface StructuringSummary {
  document_type: string;
  confidence: number;
  entities_count: number;
  has_professional: boolean;
  document_date: string | null;
}

/**
 * Progress event emitted during document processing (E2E-B05).
 *
 * Stages (in order): importing → extracting → structuring → saving_review → complete
 * On failure: failed (with error field populated)
 */
export interface ProcessingProgressEvent {
  stage: 'importing' | 'extracting' | 'structuring' | 'saving_review' | 'complete' | 'failed';
  file_name: string;
  document_id: string | null;
  progress_pct: number | null;
  error: string | null;
}

/** Batch progress event emitted during multi-file processing. */
export interface ProcessingBatchProgressEvent {
  current: number;
  total: number;
  file_name: string;
  stage: string;
}
