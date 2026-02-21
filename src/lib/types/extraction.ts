// LP-01: Night Batch Extraction — TypeScript types matching Rust backend.

/** Domain of extracted health data. */
export type ExtractionDomain = 'symptom' | 'medication' | 'appointment';

/** How well extracted data is grounded in source conversation text. */
export type Grounding = 'grounded' | 'partial' | 'ungrounded';

/** Lifecycle status of a pending review item. */
export type PendingStatus = 'pending' | 'confirmed' | 'edited_confirmed' | 'dismissed';

/** A pending extraction item awaiting user review. */
export interface PendingReviewItem {
	id: string;
	conversation_id: string;
	batch_id: string;
	domain: ExtractionDomain;
	extracted_data: Record<string, unknown>;
	confidence: number;
	grounding: Grounding;
	duplicate_of: string | null;
	source_message_ids: string[];
	status: PendingStatus;
	created_at: string;
	reviewed_at: string | null;
}

/** Result of dispatching a confirmed item to a domain table. */
export interface DispatchResult {
	item_id: string;
	domain: ExtractionDomain;
	success: boolean;
	created_record_id: string | null;
	error: string | null;
	correlations?: TemporalCorrelation[] | null;
	duplicate_warning?: string | null;
}

/** Temporal correlation between a symptom onset and a medication change. */
export interface TemporalCorrelation {
	medication_name: string;
	medication_change_date: string;
	days_since_change: number;
	message: string;
}

/** Symptom categories matching backend CATEGORIES constant. */
export const SYMPTOM_CATEGORIES = [
	'Pain',
	'Digestive',
	'Respiratory',
	'Neurological',
	'General',
	'Mood',
	'Skin',
	'Other',
] as const;

/** Medication route options. */
export const MEDICATION_ROUTES = ['oral', 'topical', 'injection', 'inhaled', 'other'] as const;

/** Severity color mappings (1-5). */
export const SEVERITY_COLORS: Record<number, { bg: string; text: string; border: string }> = {
	1: { bg: 'bg-green-100 dark:bg-green-900/30', text: 'text-green-700 dark:text-green-400', border: 'border-green-300 dark:border-green-700' },
	2: { bg: 'bg-lime-100 dark:bg-lime-900/30', text: 'text-lime-700 dark:text-lime-400', border: 'border-lime-300 dark:border-lime-700' },
	3: { bg: 'bg-amber-100 dark:bg-amber-900/30', text: 'text-amber-700 dark:text-amber-400', border: 'border-amber-300 dark:border-amber-700' },
	4: { bg: 'bg-orange-100 dark:bg-orange-900/30', text: 'text-orange-700 dark:text-orange-400', border: 'border-orange-300 dark:border-orange-700' },
	5: { bg: 'bg-red-100 dark:bg-red-900/30', text: 'text-red-700 dark:text-red-400', border: 'border-red-300 dark:border-red-700' },
};

/** Result of running a full extraction batch. */
export interface BatchResult {
	conversations_processed: number;
	conversations_skipped: number;
	items_extracted: number;
	items_stored: number;
	duration_ms: number;
	errors: string[];
}

/** Progress event emitted during batch extraction. */
export type BatchStatusEvent =
	| { Started: { conversation_count: number } }
	| { Progress: { completed: number; total: number; current_title: string } }
	| { Completed: { items_found: number; duration_ms: number } }
	| { Failed: { error: string } };

/** Domain display labels for the UI. */
export const DOMAIN_LABELS: Record<ExtractionDomain, string> = {
	symptom: 'extraction.domain_symptom',
	medication: 'extraction.domain_medication',
	appointment: 'extraction.domain_appointment',
};

/** Grounding display labels. */
export const GROUNDING_LABELS: Record<Grounding, string> = {
	grounded: 'extraction.grounding_high',
	partial: 'extraction.grounding_medium',
	ungrounded: 'extraction.grounding_low',
};
