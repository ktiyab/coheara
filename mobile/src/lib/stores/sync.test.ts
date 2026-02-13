// M0-04: Sync Manager tests — version-based sync, journal piggybacking, profile switch
import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { get } from 'svelte/store';
import {
	syncManager,
	isSyncing,
	syncStatus,
	lastSyncError,
	configureSyncManager,
	clearSyncConfig,
	requestSync,
	fullResync,
	handleProfileSwitch,
	hasUnsyncedJournal,
	startAutoSync,
	stopAutoSync,
	onAppForeground,
	onWsReconnect,
	resetSyncManagerState,
	getRetryCount
} from './sync.js';
import { syncState, resetCacheManagerState } from './cache-manager.js';
import { medications, labResults, activeAlerts, profile, lastSyncTimestamp } from './cache.js';
import { journalEntries, pendingCorrelations, resetJournalState } from './journal.js';
import { connection } from './connection.js';
import type { SyncResponse } from '$lib/types/sync.js';
import type { SyncApiResult } from '$lib/api/sync.js';

// === MOCK: postSync API ===

let mockPostSync: ReturnType<typeof vi.fn>;

vi.mock('$lib/api/sync.js', () => ({
	postSync: (...args: unknown[]) => mockPostSync(...args)
}));

// === HELPERS ===

function setConnected(): void {
	connection.set({ status: 'connected', profileName: 'Mamadou', lastSync: '2026-01-01T00:00:00Z' });
}

function setDisconnected(): void {
	connection.set({ status: 'offline', profileName: 'Mamadou', lastSync: '2026-01-01T00:00:00Z', cachedAt: 'now' });
}

function makeNoChangeResult(): SyncApiResult {
	return { status: 204 };
}

function makeSyncResponse(overrides: Partial<SyncResponse> = {}): SyncResponse {
	return {
		versions: { medications: 5, labs: 3, timeline: 10, alerts: 2, appointments: 1, profile: 1 },
		synced_at: '2026-02-12T10:00:00Z',
		...overrides
	};
}

function makeDeltaResult(overrides: Partial<SyncResponse> = {}): SyncApiResult {
	return { status: 200, data: makeSyncResponse(overrides) };
}

function makeFullSyncResult(): SyncApiResult {
	return {
		status: 200,
		data: makeSyncResponse({
			medications: [{ id: 'med-1', name: 'Metformin', dose: '500mg', frequency: 'Twice daily', prescriber: 'Dr. Ndiaye', purpose: 'Blood sugar', scheduleGroup: 'morning', since: '2025-01-01', isActive: true }],
			labs: [{ id: 'lab-1', testName: 'HbA1c', value: 7.2, unit: '%', referenceMin: 4, referenceMax: 5.6, isAbnormal: true, trend: 'up', trendContext: 'worsening', testedAt: '2025-06-01' }],
			timeline: [{ id: 'evt-1', title: 'Blood test', timestamp: '2025-06-01', eventType: 'lab_result', description: 'Routine', isPatientReported: false }],
			alerts: [{ id: 'alert-1', title: 'HbA1c rising', description: 'Trend detected', severity: 'warning', createdAt: '2025-06-01', dismissed: false }],
			appointment: { id: 'appt-1', doctorName: 'Dr. Ndiaye', date: '2026-03-01', location: 'Clinic', purpose: 'Follow-up', hasPrepData: false },
			profile: { name: 'Mamadou', blood_type: 'O+', allergies: ['Penicillin'], emergency_contacts: [{ name: 'Papa', phone: '+221', relationship: 'Father' }] },
			versions: { medications: 5, labs: 3, timeline: 10, alerts: 2, appointments: 1, profile: 1 }
		})
	};
}

function makeErrorResult(msg = 'Network error'): SyncApiResult {
	return { status: 'error', message: msg };
}

// === SETUP ===

beforeEach(() => {
	vi.useFakeTimers();
	resetSyncManagerState();
	resetCacheManagerState();
	resetJournalState();
	mockPostSync = vi.fn();
	configureSyncManager('https://desktop.local:9443', 'test-token');
	setConnected();
});

afterEach(() => {
	vi.useRealTimers();
	stopAutoSync();
});

// === INITIAL STATE ===

describe('sync-manager — initial state', () => {
	it('starts idle', () => {
		resetSyncManagerState();
		const state = get(syncManager);
		expect(state.status).toBe('idle');
		expect(state.syncInProgress).toBe(false);
		expect(state.lastSyncAt).toBeNull();
		expect(state.lastError).toBeNull();
	});

	it('derived stores reflect initial state', () => {
		resetSyncManagerState();
		expect(get(isSyncing)).toBe(false);
		expect(get(syncStatus)).toBe('idle');
		expect(get(lastSyncError)).toBeNull();
	});
});

// === NO-CHANGE SYNC (204) ===

describe('sync-manager — no-change sync', () => {
	it('handles 204 no-change response', async () => {
		mockPostSync.mockResolvedValue(makeNoChangeResult());
		const changed = await requestSync();

		expect(changed).toBe(false);
		expect(get(syncManager).status).toBe('success');
		expect(get(syncManager).lastSyncAt).toBeTruthy();
		expect(get(syncManager).syncInProgress).toBe(false);
	});

	it('sends current versions in request', async () => {
		syncState.update(($s) => ({
			...$s,
			versions: { medications: 42, labs: 15, timeline: 30, alerts: 5, appointments: 1, profile: 3 }
		}));
		mockPostSync.mockResolvedValue(makeNoChangeResult());
		await requestSync();

		expect(mockPostSync).toHaveBeenCalledWith(
			'https://desktop.local:9443',
			'test-token',
			expect.objectContaining({
				versions: { medications: 42, labs: 15, timeline: 30, alerts: 5, appointments: 1, profile: 3 }
			})
		);
	});

	it('does not modify cache stores on 204', async () => {
		mockPostSync.mockResolvedValue(makeNoChangeResult());
		await requestSync();

		expect(get(medications)).toHaveLength(0);
		expect(get(labResults)).toHaveLength(0);
	});
});

// === DELTA SYNC ===

describe('sync-manager — delta sync', () => {
	it('applies medication delta', async () => {
		// Set non-zero versions to trigger delta path
		syncState.update(($s) => ({
			...$s,
			versions: { medications: 3, labs: 1, timeline: 1, alerts: 1, appointments: 1, profile: 1 },
			cachePopulated: true
		}));

		mockPostSync.mockResolvedValue(makeDeltaResult({
			medications: [{ id: 'med-new', name: 'Aspirin', dose: '100mg', frequency: 'Daily', prescriber: 'Dr. Chen', purpose: 'Heart', scheduleGroup: 'morning', since: '2025-02-01', isActive: true }],
			versions: { medications: 4, labs: 1, timeline: 1, alerts: 1, appointments: 1, profile: 1 }
		}));

		const changed = await requestSync();
		expect(changed).toBe(true);
		expect(get(medications)).toHaveLength(1);
		expect(get(medications)[0].name).toBe('Aspirin');
	});

	it('applies multiple entity types in one sync', async () => {
		syncState.update(($s) => ({
			...$s,
			versions: { medications: 1, labs: 1, timeline: 1, alerts: 1, appointments: 1, profile: 1 },
			cachePopulated: true
		}));

		mockPostSync.mockResolvedValue(makeDeltaResult({
			medications: [{ id: 'med-1', name: 'Metformin', dose: '500mg', frequency: 'Twice daily', prescriber: 'Dr. Ndiaye', purpose: 'Blood sugar', scheduleGroup: 'morning', since: '2025-01-01', isActive: true }],
			alerts: [{ id: 'alert-1', title: 'New alert', description: 'Check this', severity: 'info', createdAt: '2026-02-12', dismissed: false }],
			versions: { medications: 2, labs: 1, timeline: 1, alerts: 2, appointments: 1, profile: 1 }
		}));

		const changed = await requestSync();
		expect(changed).toBe(true);
		expect(get(medications)).toHaveLength(1);
		expect(get(activeAlerts)).toHaveLength(1);
	});

	it('updates versions after delta sync', async () => {
		syncState.update(($s) => ({
			...$s,
			versions: { medications: 1, labs: 1, timeline: 1, alerts: 1, appointments: 1, profile: 1 },
			cachePopulated: true
		}));

		mockPostSync.mockResolvedValue(makeDeltaResult({
			versions: { medications: 5, labs: 3, timeline: 10, alerts: 2, appointments: 1, profile: 1 }
		}));

		await requestSync();

		const state = get(syncState);
		expect(state.versions.medications).toBe(5);
		expect(state.versions.labs).toBe(3);
		expect(state.versions.timeline).toBe(10);
	});
});

// === FULL SYNC ===

describe('sync-manager — full sync', () => {
	it('populates all stores on first sync (versions=0)', async () => {
		mockPostSync.mockResolvedValue(makeFullSyncResult());

		const changed = await requestSync();
		expect(changed).toBe(true);

		expect(get(medications)).toHaveLength(1);
		expect(get(medications)[0].name).toBe('Metformin');
		expect(get(labResults)).toHaveLength(1);
		expect(get(activeAlerts)).toHaveLength(1);
		expect(get(profile)).toBeTruthy();
		expect(get(profile)?.name).toBe('Mamadou');
	});

	it('updates sync state versions after full sync', async () => {
		mockPostSync.mockResolvedValue(makeFullSyncResult());
		await requestSync();

		const state = get(syncState);
		expect(state.versions.medications).toBe(5);
		expect(state.cachePopulated).toBe(true);
		expect(state.lastSyncAt).toBe('2026-02-12T10:00:00Z');
	});

	it('sets lastSyncTimestamp in cache store', async () => {
		mockPostSync.mockResolvedValue(makeFullSyncResult());
		await requestSync();

		expect(get(lastSyncTimestamp)).toBe('2026-02-12T10:00:00Z');
	});
});

// === JOURNAL PIGGYBACKING ===

describe('sync-manager — journal piggybacking', () => {
	it('includes unsynced journal entries in sync request', async () => {
		journalEntries.set([{
			id: 'j-1',
			severity: 6,
			bodyLocations: ['head'],
			freeText: 'Dizzy after walking',
			activityContext: 'Walking back from class',
			symptomChip: 'dizzy',
			oldcarts: null,
			createdAt: '2026-02-12T14:15:00Z',
			synced: false,
			syncedAt: null
		}]);

		mockPostSync.mockResolvedValue(makeNoChangeResult());
		await requestSync();

		const callArgs = mockPostSync.mock.calls[0];
		expect(callArgs[2].journal_entries).toHaveLength(1);
		expect(callArgs[2].journal_entries[0].id).toBe('j-1');
		expect(callArgs[2].journal_entries[0].body_location).toBe('head');
		expect(callArgs[2].journal_entries[0].free_text).toBe('Dizzy after walking');
		expect(callArgs[2].journal_entries[0].symptom_chip).toBe('dizzy');
	});

	it('does not include already-synced entries', async () => {
		journalEntries.set([
			{
				id: 'j-1', severity: 6, bodyLocations: ['head'], freeText: 'Old',
				activityContext: '', symptomChip: null, oldcarts: null,
				createdAt: '2026-02-10T00:00:00Z', synced: true, syncedAt: '2026-02-10T01:00:00Z'
			},
			{
				id: 'j-2', severity: 3, bodyLocations: ['chest_center'], freeText: 'New',
				activityContext: '', symptomChip: null, oldcarts: null,
				createdAt: '2026-02-12T00:00:00Z', synced: false, syncedAt: null
			}
		]);

		mockPostSync.mockResolvedValue(makeNoChangeResult());
		await requestSync();

		const callArgs = mockPostSync.mock.calls[0];
		expect(callArgs[2].journal_entries).toHaveLength(1);
		expect(callArgs[2].journal_entries[0].id).toBe('j-2');
	});

	it('marks journal entries as synced after successful sync', async () => {
		journalEntries.set([{
			id: 'j-1', severity: 6, bodyLocations: ['head'], freeText: 'Test',
			activityContext: '', symptomChip: null, oldcarts: null,
			createdAt: '2026-02-12T00:00:00Z', synced: false, syncedAt: null
		}]);

		mockPostSync.mockResolvedValue(makeDeltaResult({
			journal_sync: {
				synced_ids: ['j-1'],
				correlations: []
			}
		}));

		await requestSync();

		const entries = get(journalEntries);
		expect(entries[0].synced).toBe(true);
		expect(entries[0].syncedAt).toBeTruthy();
	});

	it('handles correlations from journal sync', async () => {
		journalEntries.set([{
			id: 'j-1', severity: 6, bodyLocations: ['head'], freeText: 'Test',
			activityContext: '', symptomChip: null, oldcarts: null,
			createdAt: '2026-02-12T00:00:00Z', synced: false, syncedAt: null
		}]);

		mockPostSync.mockResolvedValue(makeDeltaResult({
			journal_sync: {
				synced_ids: ['j-1'],
				correlations: [{
					entryId: 'j-1',
					medication: 'Aspirin',
					daysSinceChange: 2,
					message: 'Started 2 days ago'
				}]
			}
		}));

		await requestSync();

		const correlations = get(pendingCorrelations);
		expect(correlations).toHaveLength(1);
		expect(correlations[0].medication).toBe('Aspirin');
	});

	it('omits journal_entries when none are unsynced', async () => {
		mockPostSync.mockResolvedValue(makeNoChangeResult());
		await requestSync();

		const callArgs = mockPostSync.mock.calls[0];
		expect(callArgs[2].journal_entries).toBeUndefined();
	});
});

// === PROFILE SWITCH ===

describe('sync-manager — profile switch', () => {
	it('clears cache and does full resync on profile switch', async () => {
		// First populate some data
		medications.set([{ id: 'med-1', name: 'Metformin', dose: '500mg', frequency: 'Daily', prescriber: 'Dr. A', purpose: 'Blood sugar', scheduleGroup: 'morning', since: '2025-01-01', isActive: true }]);
		syncState.update(($s) => ({
			...$s,
			versions: { medications: 5, labs: 3, timeline: 10, alerts: 2, appointments: 1, profile: 1 },
			cachePopulated: true
		}));

		// Mock returns new profile data
		mockPostSync.mockResolvedValue(makeFullSyncResult());

		await handleProfileSwitch('Papa');

		// Versions should have been reset then updated
		const state = get(syncState);
		expect(state.versions.medications).toBe(5); // New profile's versions
		expect(state.cachePopulated).toBe(true);
	});

	it('detects unsynced journal entries before switch', () => {
		expect(hasUnsyncedJournal()).toBe(false);

		journalEntries.set([{
			id: 'j-1', severity: 5, bodyLocations: [], freeText: 'Test',
			activityContext: '', symptomChip: null, oldcarts: null,
			createdAt: '2026-02-12T00:00:00Z', synced: false, syncedAt: null
		}]);

		expect(hasUnsyncedJournal()).toBe(true);
	});

	it('sends versions=0 after profile switch (full resync)', async () => {
		syncState.update(($s) => ({
			...$s,
			versions: { medications: 42, labs: 15, timeline: 30, alerts: 5, appointments: 1, profile: 3 }
		}));

		mockPostSync.mockResolvedValue(makeFullSyncResult());
		await fullResync();

		const callArgs = mockPostSync.mock.calls[0];
		expect(callArgs[2].versions).toEqual({
			medications: 0, labs: 0, timeline: 0, alerts: 0, appointments: 0, profile: 0
		});
	});
});

// === GUARDS ===

describe('sync-manager — guards', () => {
	it('rejects sync when not configured', async () => {
		clearSyncConfig();
		const changed = await requestSync();
		expect(changed).toBe(false);
		expect(mockPostSync).not.toHaveBeenCalled();
	});

	it('rejects sync when disconnected', async () => {
		setDisconnected();
		const changed = await requestSync();
		expect(changed).toBe(false);
		expect(mockPostSync).not.toHaveBeenCalled();
	});

	it('prevents concurrent syncs', async () => {
		let resolveFirst: (v: SyncApiResult) => void;
		mockPostSync.mockReturnValueOnce(
			new Promise<SyncApiResult>((r) => { resolveFirst = r; })
		);

		// Start first sync (will hang)
		const first = requestSync();

		// Try second sync while first is pending
		const second = await requestSync();
		expect(second).toBe(false);

		// Resolve first
		resolveFirst!(makeNoChangeResult());
		await first;

		// Only one API call made
		expect(mockPostSync).toHaveBeenCalledTimes(1);
	});
});

// === ERROR HANDLING ===

describe('sync-manager — error handling', () => {
	it('sets error status on API error', async () => {
		mockPostSync.mockResolvedValue(makeErrorResult('Connection refused'));
		await requestSync();

		const state = get(syncManager);
		expect(state.status).toBe('error');
		expect(state.lastError).toBe('Connection refused');
		expect(state.syncInProgress).toBe(false);
	});

	it('sets error on network exception', async () => {
		mockPostSync.mockRejectedValue(new Error('fetch failed'));
		await requestSync();

		const state = get(syncManager);
		expect(state.status).toBe('error');
		expect(state.lastError).toBe('fetch failed');
		expect(state.syncInProgress).toBe(false);
	});

	it('increments retry count on error', async () => {
		mockPostSync.mockResolvedValue(makeErrorResult());

		await requestSync();
		expect(getRetryCount()).toBe(1);

		await requestSync();
		expect(getRetryCount()).toBe(2);
	});

	it('resets retry count on successful sync', async () => {
		mockPostSync.mockResolvedValue(makeErrorResult());
		await requestSync();
		expect(getRetryCount()).toBe(1);

		mockPostSync.mockResolvedValue(makeNoChangeResult());
		await requestSync();
		expect(getRetryCount()).toBe(0);
	});

	it('preserves lastSyncAt on error', async () => {
		// First successful sync
		mockPostSync.mockResolvedValue(makeNoChangeResult());
		await requestSync();
		const lastSync = get(syncManager).lastSyncAt;
		expect(lastSync).toBeTruthy();

		// Then error
		mockPostSync.mockResolvedValue(makeErrorResult());
		await requestSync();
		expect(get(syncManager).lastSyncAt).toBe(lastSync);
	});
});

// === AUTO-SYNC ===

describe('sync-manager — auto-sync', () => {
	it('triggers sync on interval', async () => {
		mockPostSync.mockResolvedValue(makeNoChangeResult());

		startAutoSync();

		// Advance by one interval
		await vi.advanceTimersByTimeAsync(5 * 60 * 1000);
		expect(mockPostSync).toHaveBeenCalledTimes(1);

		// Advance by another interval
		await vi.advanceTimersByTimeAsync(5 * 60 * 1000);
		expect(mockPostSync).toHaveBeenCalledTimes(2);

		stopAutoSync();
	});

	it('does not auto-sync when disconnected', async () => {
		setDisconnected();
		startAutoSync();

		await vi.advanceTimersByTimeAsync(5 * 60 * 1000);
		expect(mockPostSync).not.toHaveBeenCalled();

		stopAutoSync();
	});

	it('stops auto-sync when stopped', async () => {
		mockPostSync.mockResolvedValue(makeNoChangeResult());
		startAutoSync();

		await vi.advanceTimersByTimeAsync(5 * 60 * 1000);
		expect(mockPostSync).toHaveBeenCalledTimes(1);

		stopAutoSync();

		await vi.advanceTimersByTimeAsync(5 * 60 * 1000);
		expect(mockPostSync).toHaveBeenCalledTimes(1); // No additional calls
	});

	it('does not auto-sync after max retries', async () => {
		mockPostSync.mockResolvedValue(makeErrorResult());

		// Exhaust retries
		await requestSync(); // retry 1
		await requestSync(); // retry 2
		await requestSync(); // retry 3
		expect(getRetryCount()).toBe(3);

		// Now auto-sync should skip
		startAutoSync();
		await vi.advanceTimersByTimeAsync(5 * 60 * 1000);
		expect(mockPostSync).toHaveBeenCalledTimes(3); // No new call

		stopAutoSync();
	});
});

// === FOREGROUND / RECONNECT TRIGGERS ===

describe('sync-manager — trigger events', () => {
	it('triggers sync on app foreground', async () => {
		mockPostSync.mockResolvedValue(makeNoChangeResult());
		onAppForeground();
		await vi.advanceTimersByTimeAsync(0);
		expect(mockPostSync).toHaveBeenCalledTimes(1);
	});

	it('does not trigger foreground sync when disconnected', async () => {
		setDisconnected();
		onAppForeground();
		await vi.advanceTimersByTimeAsync(0);
		expect(mockPostSync).not.toHaveBeenCalled();
	});

	it('resets retries and triggers sync on WS reconnect', async () => {
		mockPostSync.mockResolvedValue(makeErrorResult());
		await requestSync();
		await requestSync();
		expect(getRetryCount()).toBe(2);

		mockPostSync.mockResolvedValue(makeNoChangeResult());
		onWsReconnect();
		await vi.advanceTimersByTimeAsync(0);
		expect(getRetryCount()).toBe(0);
	});
});

// === CONFIGURATION ===

describe('sync-manager — configuration', () => {
	it('configures base URL and token', async () => {
		configureSyncManager('https://new.local:9443', 'new-token');
		mockPostSync.mockResolvedValue(makeNoChangeResult());
		await requestSync();

		expect(mockPostSync).toHaveBeenCalledWith(
			'https://new.local:9443',
			'new-token',
			expect.any(Object)
		);
	});

	it('clears config and stops auto-sync on clearSyncConfig', async () => {
		startAutoSync();
		clearSyncConfig();

		const changed = await requestSync();
		expect(changed).toBe(false);
		expect(mockPostSync).not.toHaveBeenCalled();
	});
});
