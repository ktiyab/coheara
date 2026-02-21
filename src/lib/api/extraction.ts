// LP-01: Night Batch Extraction — Tauri invoke wrappers.

import { invoke } from '@tauri-apps/api/core';
import type {
	PendingReviewItem,
	DispatchResult,
	BatchResult,
} from '$lib/types/extraction';

/** Fetch all pending extraction items for the morning review. */
export async function getPendingExtractions(): Promise<PendingReviewItem[]> {
	return invoke<PendingReviewItem[]>('get_pending_extractions');
}

/** Get the count of pending extraction items (for badge/indicator). */
export async function getPendingExtractionCount(): Promise<number> {
	return invoke<number>('get_pending_extraction_count');
}

/** Confirm a single extraction item: dispatch to domain table. */
export async function confirmExtraction(itemId: string): Promise<DispatchResult> {
	return invoke<DispatchResult>('confirm_extraction', { item_id: itemId });
}

/** Confirm with user edits applied before dispatch. */
export async function confirmExtractionWithEdits(
	itemId: string,
	edits: Record<string, unknown>,
): Promise<DispatchResult> {
	return invoke<DispatchResult>('confirm_extraction_with_edits', {
		item_id: itemId,
		edits,
	});
}

/** Dismiss a single extraction item. */
export async function dismissExtraction(itemId: string): Promise<void> {
	return invoke('dismiss_extraction', { item_id: itemId });
}

/** Dismiss multiple extraction items at once. */
export async function dismissAllExtractions(itemIds: string[]): Promise<void> {
	return invoke('dismiss_all_extractions', { item_ids: itemIds });
}

/** Manually trigger a batch extraction run. */
export async function triggerExtractionBatch(): Promise<BatchResult> {
	return invoke<BatchResult>('trigger_extraction_batch');
}
