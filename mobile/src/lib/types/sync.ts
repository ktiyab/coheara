// M0-04: Sync Engine types — request/response, sync state, auto-sync
import type { SyncVersions } from './cache-manager.js';
import type { JournalEntry, JournalCorrelation } from './journal.js';
import type { CachedMedication, CachedLabResult, CachedTimelineEvent, CachedAlert, CachedAppointment } from './viewer.js';

// === SYNC REQUEST (phone → desktop) ===

export interface SyncRequest {
	versions: SyncVersions;
	journal_entries?: SyncJournalEntry[];
}

/** Journal entry serialized for sync (camelCase → snake_case for API) */
export interface SyncJournalEntry {
	id: string;
	severity: number;
	body_location: string;
	free_text: string;
	activity_context: string;
	symptom_chip: string | null;
	oldcarts_json: string | null;
	created_at: string;
}

// === SYNC RESPONSE (desktop → phone) ===

export interface SyncResponse {
	medications?: CachedMedication[];
	labs?: CachedLabResult[];
	timeline?: CachedTimelineEvent[];
	alerts?: CachedAlert[];
	appointment?: CachedAppointment | null;
	profile?: SyncProfile;
	versions: SyncVersions;
	synced_at: string;
	journal_sync?: JournalSyncResponse;
}

export interface SyncProfile {
	name: string;
	blood_type: string;
	allergies: string[];
	emergency_contacts: EmergencyContact[];
}

export interface EmergencyContact {
	name: string;
	phone: string;
	relationship: string;
}

export interface JournalSyncResponse {
	synced_ids: string[];
	correlations: JournalCorrelation[];
}

// === SYNC MANAGER STATE ===

export type SyncStatus = 'idle' | 'syncing' | 'success' | 'error';

export interface SyncManagerState {
	status: SyncStatus;
	lastSyncAt: string | null;
	lastError: string | null;
	syncInProgress: boolean;
}

// === PROFILE SWITCH ===

export interface ProfileSwitchEvent {
	newProfileName: string;
}

// === CONSTANTS ===

export const SYNC_INTERVAL_MS = 5 * 60 * 1000; // 5 minutes
export const SYNC_STALE_THRESHOLD_MS = 200; // <200ms for no-change sync
export const MAX_SYNC_RETRIES = 3;
