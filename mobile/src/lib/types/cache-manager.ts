// M1-06: Cache Manager types — encrypted storage interface, sync payloads, wipe protocol
import type {
	CachedMedication,
	CachedLabResult,
	CachedTimelineEvent,
	CachedAlert,
	CachedAppointment,
	CachedProfile
} from './viewer.js';

// === SYNC PAYLOAD (from desktop via M0-04) ===

export interface EmergencyContact {
	name: string;
	phone: string;
	relation: string;
}

export interface SyncProfile {
	name: string;
	blood_type?: string;
	allergies: string[];
	emergency_contacts: EmergencyContact[];
}

/** Desktop assembles this curated payload. Phone cannot request more. */
export interface SyncPayload {
	profile: SyncProfile;
	medications: CachedMedication[];
	labs: CachedLabResult[];
	timeline: CachedTimelineEvent[];
	alerts: CachedAlert[];
	appointment?: CachedAppointment;
	versions: SyncVersions;
	synced_at: string;
}

export interface SyncVersions {
	medications: number;
	labs: number;
	timeline: number;
	alerts: number;
	appointments: number;
	profile: number;
}

export function emptySyncVersions(): SyncVersions {
	return { medications: 0, labs: 0, timeline: 0, alerts: 0, appointments: 0, profile: 0 };
}

// === DELTA SYNC ===

export interface DeltaPayload {
	medications?: CachedMedication[];
	labs?: CachedLabResult[];
	timeline?: CachedTimelineEvent[];
	alerts?: CachedAlert[];
	appointment?: CachedAppointment | null;
	profile?: SyncProfile;
	removed_medication_ids?: string[];
	removed_lab_ids?: string[];
	removed_timeline_ids?: string[];
	removed_alert_ids?: string[];
	versions: SyncVersions;
	synced_at: string;
}

// === DEFERRED QUESTIONS (M1-02 offline queue) ===

export interface DeferredQuestion {
	id: string;
	questionText: string;
	createdAt: string;
	asked: boolean;
	askedAt: string | null;
}

// === SYNC STATE ===

export interface SyncState {
	versions: SyncVersions;
	lastSyncAt: string | null;
	cachePopulated: boolean;
}

// === FRESHNESS (extended from M1-03 for M1-06 spec) ===

export type FreshnessTier = 'green' | 'neutral' | 'amber' | 'red';

/** M1-06 freshness thresholds (Dr. Diallo + Lena) */
export const FRESHNESS_TIER_THRESHOLDS = {
	GREEN_MS: 2 * 60 * 60 * 1000,        // < 2 hours
	NEUTRAL_MS: 24 * 60 * 60 * 1000,     // 2 - 24 hours
	AMBER_MS: 7 * 24 * 60 * 60 * 1000,   // 24h - 7 days
	// > 7 days = red
} as const;

// === WIPE PROTOCOL ===

export type WipeTrigger =
	| 'user_unpair'
	| 'desktop_revocation'
	| 'token_expired'
	| 'profile_switch';

export type WipeScope =
	| 'full'           // DB + Keychain + Prefs (except user preferences)
	| 'health_only';   // Health data only (profile switch — keep Keychain + user prefs)

// === CACHE MANAGER INTERFACE ===

export interface CacheManager {
	// Initialization
	initialize(encryptionKey: Uint8Array): Promise<void>;
	isPopulated(): boolean;
	close(): Promise<void>;

	// Read operations (used by all viewer screens)
	getMedications(): Promise<CachedMedication[]>;
	getActiveMedications(): Promise<CachedMedication[]>;
	getLabResults(): Promise<CachedLabResult[]>;
	getTimelineEvents(limit?: number): Promise<CachedTimelineEvent[]>;
	getActiveAlerts(): Promise<CachedAlert[]>;
	getNextAppointment(): Promise<CachedAppointment | null>;
	getProfile(): Promise<CachedProfile | null>;

	// Write operations (used by sync engine)
	applySyncPayload(payload: SyncPayload): Promise<void>;
	applyDeltaPayload(payload: DeltaPayload): Promise<void>;

	// Journal
	getUnsyncedJournalEntryCount(): Promise<number>;

	// Deferred questions
	saveDeferredQuestion(question: DeferredQuestion): Promise<void>;
	getDeferredQuestions(): Promise<DeferredQuestion[]>;
	markQuestionsAsked(ids: string[]): Promise<void>;
	clearDeferredQuestions(ids: string[]): Promise<void>;

	// Sync state
	getSyncVersions(): Promise<SyncVersions>;
	getLastSyncTimestamp(): Promise<string | null>;

	// Lifecycle
	wipe(scope: WipeScope): Promise<void>;
	cleanupExpiredAppointment(): Promise<void>;
}

// === FIRST SYNC PROGRESS ===

export type FirstSyncStage =
	| 'connecting'
	| 'downloading'
	| 'storing'
	| 'complete';

export interface FirstSyncProgress {
	stage: FirstSyncStage;
	percent: number;
}
