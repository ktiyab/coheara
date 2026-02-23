// M0-04: Sync Manager tests — version-based sync, profile switch (aligned CA-05)
import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { get } from 'svelte/store';
import {
	syncManager,
	isSyncing,
	syncStatus,
	lastSyncError,
	syncAuditLog,
	configureSyncManager,
	clearSyncConfig,
	requestSync,
	fullResync,
	handleProfileSwitch,
	startAutoSync,
	stopAutoSync,
	onAppForeground,
	onWsReconnect,
	resetSyncManagerState,
	getRetryCount
} from './sync.js';
import { syncState, resetCacheManagerState } from './cache-manager.js';
import { medications, labResults, activeAlerts, profile, lastSyncTimestamp } from './cache.js';
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
		syncedAt: '2026-02-12T10:00:00Z',
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
			medications: [{ id: 'med-1', genericName: 'Metformin', dose: '500mg', frequency: 'Twice daily', route: 'oral', status: 'active', isOtc: false }],
			labs: [{ id: 'lab-1', testName: 'HbA1c', value: 7.2, unit: '%', referenceRangeLow: 4, referenceRangeHigh: 5.6, abnormalFlag: 'H', isAbnormal: true, collectionDate: '2025-06-01', trendDirection: 'up' }],
			timeline: [{ id: 'evt-1', eventType: 'lab_result', category: 'Lab Results', description: 'Routine blood test', date: '2025-06-01', stillActive: false }],
			alerts: [{ id: 'alert-1', title: 'HbA1c rising', description: 'Trend detected', severity: 'warning', createdAt: '2025-06-01', dismissed: false }],
			appointment: { id: 'appt-1', professionalName: 'Dr. Ndiaye', date: '2026-03-01', appointmentType: 'Follow-up', prepAvailable: false },
			profile: { profileName: 'Mamadou', totalDocuments: 12, extractionAccuracy: 0.92, allergies: [{ allergen: 'Penicillin', severity: 'high', verified: true }] },
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
		syncState.update(($s) => ({
			...$s,
			versions: { medications: 3, labs: 1, timeline: 1, alerts: 1, appointments: 1, profile: 1 },
			cachePopulated: true
		}));

		mockPostSync.mockResolvedValue(makeDeltaResult({
			medications: [{ id: 'med-new', genericName: 'Aspirin', dose: '100mg', frequency: 'Daily', route: 'oral', status: 'active', isOtc: true }],
			versions: { medications: 4, labs: 1, timeline: 1, alerts: 1, appointments: 1, profile: 1 }
		}));

		const changed = await requestSync();
		expect(changed).toBe(true);
		expect(get(medications)).toHaveLength(1);
		expect(get(medications)[0].genericName).toBe('Aspirin');
	});

	it('applies multiple entity types in one sync', async () => {
		syncState.update(($s) => ({
			...$s,
			versions: { medications: 1, labs: 1, timeline: 1, alerts: 1, appointments: 1, profile: 1 },
			cachePopulated: true
		}));

		mockPostSync.mockResolvedValue(makeDeltaResult({
			medications: [{ id: 'med-1', genericName: 'Metformin', dose: '500mg', frequency: 'Twice daily', route: 'oral', status: 'active', isOtc: false }],
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
		expect(get(medications)[0].genericName).toBe('Metformin');
		expect(get(labResults)).toHaveLength(1);
		expect(get(activeAlerts)).toHaveLength(1);
		expect(get(profile)).toBeTruthy();
		expect(get(profile)?.profileName).toBe('Mamadou');
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

// === PROFILE SWITCH ===

describe('sync-manager — profile switch', () => {
	it('clears cache and does full resync on profile switch', async () => {
		medications.set([{ id: 'med-1', genericName: 'Metformin', dose: '500mg', frequency: 'Daily', route: 'oral', status: 'active', isOtc: false }]);
		syncState.update(($s) => ({
			...$s,
			versions: { medications: 5, labs: 3, timeline: 10, alerts: 2, appointments: 1, profile: 1 },
			cachePopulated: true
		}));

		mockPostSync.mockResolvedValue(makeFullSyncResult());

		await handleProfileSwitch('Papa');

		const state = get(syncState);
		expect(state.versions.medications).toBe(5);
		expect(state.cachePopulated).toBe(true);
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

		const first = requestSync();

		const second = await requestSync();
		expect(second).toBe(false);

		resolveFirst!(makeNoChangeResult());
		await first;

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
		mockPostSync.mockResolvedValue(makeNoChangeResult());
		await requestSync();
		const lastSync = get(syncManager).lastSyncAt;
		expect(lastSync).toBeTruthy();

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

		await vi.advanceTimersByTimeAsync(5 * 60 * 1000);
		expect(mockPostSync).toHaveBeenCalledTimes(1);

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
		expect(mockPostSync).toHaveBeenCalledTimes(1);
	});

	it('does not auto-sync after max retries', async () => {
		mockPostSync.mockResolvedValue(makeErrorResult());

		await requestSync();
		await requestSync();
		await requestSync();
		expect(getRetryCount()).toBe(3);

		startAutoSync();
		await vi.advanceTimersByTimeAsync(5 * 60 * 1000);
		expect(mockPostSync).toHaveBeenCalledTimes(3);

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

// === SYNC TIMESTAMP VALIDATION (RS-M0-04-P01) ===

describe('sync-manager — timestamp validation', () => {
	it('rejects sync response with future timestamp', async () => {
		const futureDate = new Date(Date.now() + 60 * 60 * 1000).toISOString();
		mockPostSync.mockResolvedValue(makeDeltaResult({ syncedAt: futureDate }));

		const changed = await requestSync();
		expect(changed).toBe(false);
		expect(get(syncManager).status).toBe('error');
		expect(get(syncManager).lastError).toContain('timestamp');
	});

	it('rejects sync response older than last sync', async () => {
		mockPostSync.mockResolvedValue(makeDeltaResult({ syncedAt: '2026-02-12T10:00:00Z' }));
		await requestSync();
		expect(get(syncManager).lastSyncAt).toBe('2026-02-12T10:00:00Z');

		mockPostSync.mockResolvedValue(makeDeltaResult({ syncedAt: '2026-02-12T09:00:00Z' }));
		const changed = await requestSync();
		expect(changed).toBe(false);
		expect(get(syncManager).status).toBe('error');
	});

	it('accepts valid timestamp newer than last sync', async () => {
		mockPostSync.mockResolvedValue(makeDeltaResult({ syncedAt: '2026-02-12T10:00:00Z' }));
		await requestSync();

		mockPostSync.mockResolvedValue(makeDeltaResult({ syncedAt: '2026-02-12T11:00:00Z' }));
		const changed = await requestSync();
		expect(changed).toBe(true);
		expect(get(syncManager).lastSyncAt).toBe('2026-02-12T11:00:00Z');
	});

	it('rejects sync response with invalid date string', async () => {
		mockPostSync.mockResolvedValue(makeDeltaResult({ syncedAt: 'not-a-date' }));

		const changed = await requestSync();
		expect(changed).toBe(false);
		expect(get(syncManager).status).toBe('error');
	});
});

// === SYNC AUDIT LOGGING (RS-M0-04-P02) ===

describe('sync-manager — audit logging', () => {
	it('logs sync_applied on successful 200 sync', async () => {
		mockPostSync.mockResolvedValue(makeFullSyncResult());
		await requestSync();

		const log = get(syncAuditLog);
		expect(log).toHaveLength(1);
		expect(log[0].action).toBe('sync_applied');
		expect(log[0].entitiesUpdated).toContain('medications');
		expect(log[0].entitiesUpdated).toContain('labs');
		expect(log[0].entitiesUpdated).toContain('profile');
		expect(log[0].timestamp).toBe('2026-02-12T10:00:00Z');
	});

	it('logs sync_no_change on 204 response', async () => {
		mockPostSync.mockResolvedValue(makeNoChangeResult());
		await requestSync();

		const log = get(syncAuditLog);
		expect(log).toHaveLength(1);
		expect(log[0].action).toBe('sync_no_change');
		expect(log[0].entitiesUpdated).toHaveLength(0);
	});

	it('logs sync_error on error response', async () => {
		mockPostSync.mockResolvedValue(makeErrorResult('Server error'));
		await requestSync();

		const log = get(syncAuditLog);
		expect(log).toHaveLength(1);
		expect(log[0].action).toBe('sync_error');
		expect(log[0].error).toBe('Server error');
	});

	it('accumulates multiple audit entries', async () => {
		mockPostSync.mockResolvedValue(makeNoChangeResult());
		await requestSync();

		const futureTime = new Date(Date.now() + 60_000).toISOString();
		const full = makeFullSyncResult();
		const fullData = full.status === 200 ? full.data : makeSyncResponse();
		mockPostSync.mockResolvedValue(makeDeltaResult({
			...fullData,
			syncedAt: futureTime
		}));
		await requestSync();

		const log = get(syncAuditLog);
		expect(log).toHaveLength(2);
		expect(log[0].action).toBe('sync_no_change');
		expect(log[1].action).toBe('sync_applied');
	});

	it('clears audit log on reset', async () => {
		mockPostSync.mockResolvedValue(makeNoChangeResult());
		await requestSync();
		expect(get(syncAuditLog)).toHaveLength(1);

		resetSyncManagerState();
		configureSyncManager('https://desktop.local:9443', 'test-token');
		setConnected();
		expect(get(syncAuditLog)).toHaveLength(0);
	});
});
