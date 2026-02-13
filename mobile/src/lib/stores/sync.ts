// M0-04: Sync Manager — orchestrates version-based delta sync with desktop
import { writable, derived, get } from 'svelte/store';
import type { SyncManagerState, SyncResponse, SyncJournalEntry } from '$lib/types/sync.js';
import { SYNC_INTERVAL_MS, MAX_SYNC_RETRIES } from '$lib/types/sync.js';
import type { SyncVersions } from '$lib/types/cache-manager.js';
import { emptySyncVersions } from '$lib/types/cache-manager.js';
import { postSync, type SyncApiResult } from '$lib/api/sync.js';
import { syncState, applySyncPayload, applyDeltaPayload, wipeCache } from './cache-manager.js';
import { getUnsyncedEntries, markSynced, setCorrelations } from './journal.js';
import { isConnected } from './connection.js';
import type { JournalEntry } from '$lib/types/journal.js';
import type { DeltaPayload } from '$lib/types/cache-manager.js';

// === STATE ===

export const syncManager = writable<SyncManagerState>({
	status: 'idle',
	lastSyncAt: null,
	lastError: null,
	syncInProgress: false
});

// === DERIVED ===

export const isSyncing = derived(syncManager, ($s) => $s.syncInProgress);
export const syncStatus = derived(syncManager, ($s) => $s.status);
export const lastSyncError = derived(syncManager, ($s) => $s.lastError);

// === INTERNAL STATE ===

let autoSyncTimer: ReturnType<typeof setInterval> | null = null;
let retryCount = 0;

// === CONFIG (injectable for testing) ===

interface SyncConfig {
	baseUrl: string;
	token: string;
}

let currentConfig: SyncConfig | null = null;

/** Set sync configuration (called after pairing) */
export function configureSyncManager(baseUrl: string, token: string): void {
	currentConfig = { baseUrl, token };
	retryCount = 0;
}

/** Clear sync configuration (called on unpair) */
export function clearSyncConfig(): void {
	currentConfig = null;
	stopAutoSync();
}

// === JOURNAL SERIALIZATION ===

/** Convert local journal entries to sync wire format (camelCase → snake_case) */
function serializeJournalEntries(entries: JournalEntry[]): SyncJournalEntry[] {
	return entries.map((e) => ({
		id: e.id,
		severity: e.severity,
		body_location: e.bodyLocations.join(','),
		free_text: e.freeText,
		activity_context: e.activityContext,
		symptom_chip: e.symptomChip ?? null,
		oldcarts_json: e.oldcarts ? JSON.stringify(e.oldcarts) : null,
		created_at: e.createdAt
	}));
}

// === APPLY SYNC RESPONSE ===

/** Apply a sync response payload to local cache stores */
function applySyncResponse(response: SyncResponse): void {
	const currentSync = get(syncState);

	// Check if this is a full sync (all versions were 0) or delta
	const isFullSync = Object.values(currentSync.versions).every((v) => v === 0);

	if (isFullSync && response.profile) {
		// Full sync: use applySyncPayload which replaces everything
		applySyncPayload({
			profile: {
				name: response.profile.name,
				blood_type: response.profile.blood_type,
				allergies: response.profile.allergies,
				emergency_contacts: response.profile.emergency_contacts.map((c) => ({
					name: c.name,
					phone: c.phone,
					relation: c.relationship
				}))
			},
			medications: response.medications ?? [],
			labs: response.labs ?? [],
			timeline: response.timeline ?? [],
			alerts: response.alerts ?? [],
			appointment: response.appointment ?? undefined,
			versions: response.versions,
			synced_at: response.synced_at
		});
	} else {
		// Delta sync: use applyDeltaPayload which merges
		const delta: DeltaPayload = {
			medications: response.medications,
			labs: response.labs,
			timeline: response.timeline,
			alerts: response.alerts,
			appointment: response.appointment,
			profile: response.profile ? {
				name: response.profile.name,
				blood_type: response.profile.blood_type,
				allergies: response.profile.allergies,
				emergency_contacts: response.profile.emergency_contacts.map((c) => ({
					name: c.name,
					phone: c.phone,
					relation: c.relationship
				}))
			} : undefined,
			versions: response.versions,
			synced_at: response.synced_at
		};
		applyDeltaPayload(delta);
	}
}

// === CORE SYNC ===

/**
 * Execute one sync cycle.
 * Returns true if changes were applied, false if no-change (204) or error.
 */
export async function requestSync(): Promise<boolean> {
	// Guard: no config
	if (!currentConfig) return false;

	// Guard: already syncing
	const state = get(syncManager);
	if (state.syncInProgress) return false;

	// Guard: not connected
	if (!get(isConnected)) return false;

	// Mark in progress
	syncManager.update(($s) => ({
		...$s,
		status: 'syncing',
		syncInProgress: true
	}));

	try {
		// 1. Get local versions
		const versions: SyncVersions = get(syncState).versions;

		// 2. Get unsynced journal entries
		const unsyncedEntries = getUnsyncedEntries();
		const journalPayload = unsyncedEntries.length > 0
			? serializeJournalEntries(unsyncedEntries)
			: undefined;

		// 3. POST to desktop
		const result: SyncApiResult = await postSync(
			currentConfig.baseUrl,
			currentConfig.token,
			{
				versions,
				journal_entries: journalPayload
			}
		);

		// 4. Handle response
		if (result.status === 204) {
			// No changes — update timestamp only
			const now = new Date().toISOString();
			syncManager.set({
				status: 'success',
				lastSyncAt: now,
				lastError: null,
				syncInProgress: false
			});
			retryCount = 0;
			return false;
		}

		if (result.status === 200) {
			const response = result.data;

			// 5. Apply payload to cache
			applySyncResponse(response);

			// 6. Mark journal entries as synced
			if (response.journal_sync?.synced_ids.length) {
				markSynced(response.journal_sync.synced_ids);
			}

			// 7. Set correlations for toast display
			if (response.journal_sync?.correlations.length) {
				setCorrelations(response.journal_sync.correlations);
			}

			// 8. Update sync manager state
			syncManager.set({
				status: 'success',
				lastSyncAt: response.synced_at,
				lastError: null,
				syncInProgress: false
			});
			retryCount = 0;
			return true;
		}

		// Error response
		const errorMsg = result.status === 'error' ? result.message : 'Unknown error';
		syncManager.set({
			status: 'error',
			lastSyncAt: state.lastSyncAt,
			lastError: errorMsg,
			syncInProgress: false
		});
		retryCount++;
		return false;
	} catch (err) {
		// Network or unexpected error
		syncManager.update(($s) => ({
			...$s,
			status: 'error',
			lastError: err instanceof Error ? err.message : 'Sync failed',
			syncInProgress: false
		}));
		retryCount++;
		return false;
	}
}

// === FULL RESYNC ===

/** Clear health data and perform full sync (profile switch, cache corruption) */
export async function fullResync(): Promise<boolean> {
	// Clear health data but keep auth/keys
	wipeCache('health_only');

	// Reset versions to force full payload
	syncState.set({
		versions: emptySyncVersions(),
		lastSyncAt: null,
		cachePopulated: false
	});

	// Run sync — desktop will see all versions=0 and send everything
	return requestSync();
}

// === PROFILE SWITCH ===

/** Handle profile switch notification from desktop */
export async function handleProfileSwitch(newProfileName: string): Promise<boolean> {
	// Warn about unsynced journal entries (caller handles UI)
	// Proceed with full resync for new profile
	return fullResync();
}

/** Check if there are unsynced journal entries (for pre-switch warning) */
export function hasUnsyncedJournal(): boolean {
	return getUnsyncedEntries().length > 0;
}

// === AUTO-SYNC ===

/** Start periodic auto-sync (every 5 minutes) */
export function startAutoSync(): void {
	if (autoSyncTimer) return;
	autoSyncTimer = setInterval(() => {
		if (get(isConnected) && retryCount < MAX_SYNC_RETRIES) {
			requestSync();
		}
	}, SYNC_INTERVAL_MS);
}

/** Stop periodic auto-sync */
export function stopAutoSync(): void {
	if (autoSyncTimer) {
		clearInterval(autoSyncTimer);
		autoSyncTimer = null;
	}
}

/** Trigger sync on app foreground */
export function onAppForeground(): void {
	if (get(isConnected) && retryCount < MAX_SYNC_RETRIES) {
		requestSync();
	}
}

/** Trigger sync on WebSocket reconnect */
export function onWsReconnect(): void {
	retryCount = 0; // Reset retries on reconnect
	requestSync();
}

// === RESET (for testing) ===

export function resetSyncManagerState(): void {
	stopAutoSync();
	currentConfig = null;
	retryCount = 0;
	syncManager.set({
		status: 'idle',
		lastSyncAt: null,
		lastError: null,
		syncInProgress: false
	});
}

/** Get current retry count (for testing) */
export function getRetryCount(): number {
	return retryCount;
}
