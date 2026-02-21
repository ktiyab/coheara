// LP-01: Night Batch Extraction — reactive store for pending review items.

import {
	getPendingExtractions,
	getPendingExtractionCount,
	confirmExtraction,
	confirmExtractionWithEdits,
	dismissExtraction,
	dismissAllExtractions,
} from '$lib/api/extraction';
import type { PendingReviewItem, DispatchResult, BatchStatusEvent } from '$lib/types/extraction';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { isTauriEnv } from '$lib/utils/tauri';

/** Batch processing state visible in AiStatusIndicator. */
export interface BatchProgress {
	running: boolean;
	completed: number;
	total: number;
	currentTitle: string;
}

class ExtractionStore {
	items = $state<PendingReviewItem[]>([]);
	count = $state(0);
	loading = $state(false);
	error = $state<string | null>(null);

	/** Batch processing progress (for AiStatusIndicator). */
	batch = $state<BatchProgress>({ running: false, completed: 0, total: 0, currentTitle: '' });

	private _unlisten: UnlistenFn | null = null;

	/** Start listening to extraction-progress Tauri events. Call once at app startup. */
	async startListening(): Promise<void> {
		if (!isTauriEnv() || this._unlisten) return;
		this._unlisten = await listen<BatchStatusEvent>('extraction-progress', (event) => {
			this.handleBatchEvent(event.payload);
		});
	}

	/** Stop listening. Call on cleanup. */
	stopListening(): void {
		this._unlisten?.();
		this._unlisten = null;
	}

	private handleBatchEvent(event: BatchStatusEvent): void {
		if ('Started' in event) {
			this.batch = {
				running: true,
				completed: 0,
				total: event.Started.conversation_count,
				currentTitle: '',
			};
		} else if ('Progress' in event) {
			this.batch = {
				running: true,
				completed: event.Progress.completed,
				total: event.Progress.total,
				currentTitle: event.Progress.current_title,
			};
		} else if ('Completed' in event) {
			this.batch = { running: false, completed: 0, total: 0, currentTitle: '' };
			// Auto-refresh items when batch completes — new items may be available
			this.refresh().catch(() => {});
		} else if ('Failed' in event) {
			this.batch = { running: false, completed: 0, total: 0, currentTitle: '' };
		}
	}

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
