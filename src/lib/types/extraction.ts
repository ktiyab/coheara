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
}

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
	| { Completed: { items_found: number; duration_ms: number } };

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
