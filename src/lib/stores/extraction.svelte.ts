// LP-01: Night Batch Extraction — reactive store for pending review items.

import {
	getPendingExtractions,
	getPendingExtractionCount,
	confirmExtraction,
	confirmExtractionWithEdits,
	dismissExtraction,
	dismissAllExtractions,
} from '$lib/api/extraction';
import type { PendingReviewItem, DispatchResult } from '$lib/types/extraction';

class ExtractionStore {
	items = $state<PendingReviewItem[]>([]);
	count = $state(0);
	loading = $state(false);
	error = $state<string | null>(null);

	/** Fetch all pending items and update count. */
	async refresh(): Promise<void> {
		try {
			this.loading = true;
			this.error = null;
			this.items = await getPendingExtractions();
			this.count = this.items.length;
		} catch (e) {
			this.error = e instanceof Error ? e.message : String(e);
		} finally {
			this.loading = false;
		}
	}

	/** Lightweight count-only refresh (for badge updates). */
	async updateCount(): Promise<void> {
		try {
			this.count = await getPendingExtractionCount();
		} catch {
			// Silently fail — badge not critical
		}
	}

	/** Confirm a pending item and remove from local list. */
	async confirm(itemId: string): Promise<DispatchResult> {
		const result = await confirmExtraction(itemId);
		this.items = this.items.filter((i) => i.id !== itemId);
		this.count = this.items.length;
		return result;
	}

	/** Confirm with edits and remove from local list. */
	async confirmWithEdits(
		itemId: string,
		edits: Record<string, unknown>,
	): Promise<DispatchResult> {
		const result = await confirmExtractionWithEdits(itemId, edits);
		this.items = this.items.filter((i) => i.id !== itemId);
		this.count = this.items.length;
		return result;
	}

	/** Dismiss a single item and remove from local list. */
	async dismiss(itemId: string): Promise<void> {
		await dismissExtraction(itemId);
		this.items = this.items.filter((i) => i.id !== itemId);
		this.count = this.items.length;
	}

	/** Dismiss all current pending items. */
	async dismissAll(): Promise<void> {
		const ids = this.items.map((i) => i.id);
		if (ids.length === 0) return;
		await dismissAllExtractions(ids);
		this.items = [];
		this.count = 0;
	}
}

export const extraction = new ExtractionStore();
