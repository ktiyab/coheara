// M1-04: Journal API — sync to desktop
import { apiClient } from './client.js';
import type { JournalEntry, JournalSyncResult } from '$lib/types/journal.js';

/** Sync unsynced journal entries to desktop */
export async function syncJournalEntries(entries: JournalEntry[]): Promise<JournalSyncResult | null> {
	const payload = entries.map((e) => ({
		id: e.id,
		severity: e.severity,
		body_locations: e.bodyLocations,
		free_text: e.freeText,
		activity_context: e.activityContext,
		symptom_chip: e.symptomChip,
		oldcarts: e.oldcarts,
		created_at: e.createdAt
	}));

	const response = await apiClient.post<JournalSyncResult>('/api/journal/sync', { entries: payload });
	return response.ok && response.data ? response.data : null;
}
