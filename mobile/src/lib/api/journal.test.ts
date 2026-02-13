// M1-04: Journal API tests — 8 tests
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { syncJournalEntries } from './journal.js';
import type { JournalEntry, JournalSyncResult } from '$lib/types/journal.js';

// Mock the apiClient
vi.mock('./client.js', () => ({
	apiClient: {
		post: vi.fn()
	}
}));

import { apiClient } from './client.js';

const mockPost = vi.mocked(apiClient.post);

function makeEntry(overrides: Partial<JournalEntry> = {}): JournalEntry {
	return {
		id: 'journal-1',
		severity: 6,
		bodyLocations: ['abdomen_upper'],
		freeText: 'Stomach pain',
		activityContext: 'After eating',
		symptomChip: 'pain',
		oldcarts: { onset: { quick: 'today' } },
		createdAt: '2025-01-15T10:00:00Z',
		synced: false,
		syncedAt: null,
		...overrides
	};
}

describe('journal API — syncJournalEntries', () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it('transforms camelCase fields to snake_case for desktop API', async () => {
		mockPost.mockResolvedValue({ ok: true, status: 200, data: { syncedIds: ['journal-1'], correlations: [] } });

		await syncJournalEntries([makeEntry()]);

		expect(mockPost).toHaveBeenCalledWith('/api/journal/sync', {
			entries: [{
				id: 'journal-1',
				severity: 6,
				body_locations: ['abdomen_upper'],
				free_text: 'Stomach pain',
				activity_context: 'After eating',
				symptom_chip: 'pain',
				oldcarts: { onset: { quick: 'today' } },
				created_at: '2025-01-15T10:00:00Z'
			}]
		});
	});

	it('returns sync result on success', async () => {
		const syncResult: JournalSyncResult = {
			syncedIds: ['journal-1', 'journal-2'],
			correlations: [{
				entryId: 'journal-1',
				medication: 'Lisinopril',
				daysSinceChange: 3,
				message: 'Dizziness may be related to Lisinopril change.'
			}]
		};
		mockPost.mockResolvedValue({ ok: true, status: 200, data: syncResult });

		const result = await syncJournalEntries([makeEntry()]);

		expect(result).toEqual(syncResult);
	});

	it('returns null on HTTP error', async () => {
		mockPost.mockResolvedValue({ ok: false, status: 500, error: 'Server error' });

		const result = await syncJournalEntries([makeEntry()]);

		expect(result).toBeNull();
	});

	it('returns null when response has no data', async () => {
		mockPost.mockResolvedValue({ ok: true, status: 200 });

		const result = await syncJournalEntries([makeEntry()]);

		expect(result).toBeNull();
	});

	it('sends multiple entries in a single request', async () => {
		mockPost.mockResolvedValue({ ok: true, status: 200, data: { syncedIds: ['j1', 'j2'], correlations: [] } });

		const entries = [
			makeEntry({ id: 'j1', severity: 4 }),
			makeEntry({ id: 'j2', severity: 8 })
		];
		await syncJournalEntries(entries);

		const call = mockPost.mock.calls[0];
		const payload = call[1] as { entries: unknown[] };
		expect(payload.entries).toHaveLength(2);
	});

	it('handles null symptomChip and oldcarts', async () => {
		mockPost.mockResolvedValue({ ok: true, status: 200, data: { syncedIds: ['j1'], correlations: [] } });

		await syncJournalEntries([makeEntry({ symptomChip: null, oldcarts: null })]);

		const call = mockPost.mock.calls[0];
		const payload = call[1] as { entries: Array<Record<string, unknown>> };
		expect(payload.entries[0].symptom_chip).toBeNull();
		expect(payload.entries[0].oldcarts).toBeNull();
	});

	it('handles empty entry array', async () => {
		mockPost.mockResolvedValue({ ok: true, status: 200, data: { syncedIds: [], correlations: [] } });

		const result = await syncJournalEntries([]);

		expect(mockPost).toHaveBeenCalledWith('/api/journal/sync', { entries: [] });
		expect(result).toEqual({ syncedIds: [], correlations: [] });
	});

	it('maps all body locations without modification', async () => {
		mockPost.mockResolvedValue({ ok: true, status: 200, data: { syncedIds: ['j1'], correlations: [] } });

		const locations = ['chest_left', 'arm_left', 'neck'] as const;
		await syncJournalEntries([makeEntry({ bodyLocations: [...locations] })]);

		const call = mockPost.mock.calls[0];
		const payload = call[1] as { entries: Array<Record<string, unknown>> };
		expect(payload.entries[0].body_locations).toEqual(['chest_left', 'arm_left', 'neck']);
	});
});
