// M0-04: Sync Engine types — aligned with desktop sync.rs (CA-05)
import type { SyncVersions } from './cache-manager.js';
import type { CachedMedication, CachedLabResult, CachedTimelineEvent, CachedAlert, CachedAppointment, CachedProfile } from './viewer.js';

// === SYNC REQUEST (phone → desktop) ===

export interface SyncRequest {
	versions: SyncVersions;
}

// === SYNC RESPONSE (desktop → phone) — matches desktop SyncResponse ===

export interface SyncResponse {
	medications?: CachedMedication[];
	labs?: CachedLabResult[];
	timeline?: CachedTimelineEvent[];
	alerts?: CachedAlert[];
	appointment?: CachedAppointment;
	profile?: CachedProfile;
	versions: SyncVersions;
	syncedAt: string;
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

// === AUDIT LOGGING (RS-M0-04-P02) ===

export interface SyncAuditEntry {
	action: 'sync_applied' | 'sync_no_change' | 'sync_error';
	entitiesUpdated: string[];
	newVersions: SyncVersions;
	timestamp: string;
	error?: string;
}

// === CONSTANTS ===

export const SYNC_INTERVAL_MS = 5 * 60 * 1000; // 5 minutes
export const SYNC_STALE_THRESHOLD_MS = 200; // <200ms for no-change sync
export const MAX_SYNC_RETRIES = 3;
