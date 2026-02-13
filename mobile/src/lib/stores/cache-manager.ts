// M1-06: Cache Manager — in-memory implementation bridging to Svelte stores
// In production: would use SQLCipher/capacitor-sqlite. This implements the
// CacheManager interface using Svelte writable stores (testable, reactive).
import { writable, derived, get } from 'svelte/store';
import type {
	SyncPayload,
	DeltaPayload,
	SyncVersions,
	SyncState,
	DeferredQuestion,
	WipeScope,
	FirstSyncProgress,
	FirstSyncStage,
	FreshnessTier
} from '$lib/types/cache-manager.js';
import { emptySyncVersions, FRESHNESS_TIER_THRESHOLDS } from '$lib/types/cache-manager.js';
import type { CachedMedication, CachedLabResult, CachedTimelineEvent, CachedAlert, CachedAppointment, CachedProfile } from '$lib/types/viewer.js';
import {
	medications,
	labResults,
	timelineEvents,
	activeAlerts,
	nextAppointment,
	profile,
	lastSyncTimestamp,
	loadCacheData,
	clearCacheStores
} from './cache.js';

// === SYNC STATE ===

export const syncState = writable<SyncState>({
	versions: emptySyncVersions(),
	lastSyncAt: null,
	cachePopulated: false
});

export const cachePopulated = derived(syncState, ($s) => $s.cachePopulated);

// === DEFERRED QUESTIONS ===

export const deferredQuestions = writable<DeferredQuestion[]>([]);

export const pendingQuestionCount = derived(deferredQuestions, ($q) =>
	$q.filter((q) => !q.asked).length
);

// === FIRST SYNC PROGRESS ===

export const firstSyncProgress = writable<FirstSyncProgress | null>(null);
export const isFirstSyncing = derived(firstSyncProgress, ($p) => $p !== null && $p.stage !== 'complete');

// === FRESHNESS TIER (extended from M1-03) ===

/** Compute 4-tier freshness: green/neutral/amber/red (Dr. Diallo + Lena) */
export function computeFreshnessTier(syncTimestamp: string | null, now?: number): FreshnessTier {
	if (!syncTimestamp) return 'red';

	const elapsed = (now ?? Date.now()) - new Date(syncTimestamp).getTime();

	if (elapsed < FRESHNESS_TIER_THRESHOLDS.GREEN_MS) return 'green';
	if (elapsed < FRESHNESS_TIER_THRESHOLDS.NEUTRAL_MS) return 'neutral';
	if (elapsed < FRESHNESS_TIER_THRESHOLDS.AMBER_MS) return 'amber';
	return 'red';
}

/** Get freshness tier display label */
export function freshnessTierLabel(tier: FreshnessTier): string {
	switch (tier) {
		case 'green': return 'Up to date';
		case 'neutral': return 'Recent';
		case 'amber': return 'Some information may have changed';
		case 'red': return 'This data may be outdated';
	}
}

/** Get freshness tier CSS color */
export function freshnessTierColor(tier: FreshnessTier): string {
	switch (tier) {
		case 'green': return 'var(--color-success)';
		case 'neutral': return 'var(--color-text-muted)';
		case 'amber': return 'var(--color-warning)';
		case 'red': return 'var(--color-error)';
	}
}

/** Whether to show a staleness warning banner */
export function shouldShowStalenessWarning(tier: FreshnessTier): boolean {
	return tier === 'amber' || tier === 'red';
}

/** Get staleness warning message */
export function stalenessWarningMessage(tier: FreshnessTier): string {
	if (tier === 'amber') {
		return 'Some information may have changed. Connect to your desktop to update.';
	}
	if (tier === 'red') {
		return 'This data may be outdated. Connect to your desktop to refresh.';
	}
	return '';
}

// === SYNC PAYLOAD APPLICATION ===

/** Apply a full sync payload (first sync or full re-sync) */
export function applySyncPayload(payload: SyncPayload): void {
	// Map SyncProfile to CachedProfile (RS-M1-06-003: preserve emergency contacts)
	const mappedProfile: CachedProfile = {
		name: payload.profile.name,
		bloodType: payload.profile.blood_type,
		allergies: payload.profile.allergies,
		emergencyContacts: payload.profile.emergency_contacts.map((c) => ({
			name: c.name,
			phone: c.phone,
			relation: c.relation
		}))
	};

	loadCacheData({
		medications: payload.medications,
		labResults: payload.labs,
		timelineEvents: payload.timeline,
		alerts: payload.alerts,
		appointment: payload.appointment ?? null,
		profile: mappedProfile,
		syncTimestamp: payload.synced_at
	});

	syncState.set({
		versions: payload.versions,
		lastSyncAt: payload.synced_at,
		cachePopulated: true
	});
}

/** Apply a delta sync payload (only changed entities) */
export function applyDeltaPayload(payload: DeltaPayload): void {
	// Update medications
	if (payload.medications && payload.medications.length > 0) {
		medications.update(($meds) => {
			const updated = [...$meds];
			for (const newMed of payload.medications!) {
				const idx = updated.findIndex((m) => m.id === newMed.id);
				if (idx >= 0) updated[idx] = newMed;
				else updated.push(newMed);
			}
			return updated;
		});
	}
	if (payload.removed_medication_ids && payload.removed_medication_ids.length > 0) {
		medications.update(($meds) =>
			$meds.filter((m) => !payload.removed_medication_ids!.includes(m.id))
		);
	}

	// Update labs
	if (payload.labs && payload.labs.length > 0) {
		labResults.update(($labs) => {
			const updated = [...$labs];
			for (const newLab of payload.labs!) {
				const idx = updated.findIndex((l) => l.id === newLab.id);
				if (idx >= 0) updated[idx] = newLab;
				else updated.push(newLab);
			}
			return updated;
		});
	}
	if (payload.removed_lab_ids && payload.removed_lab_ids.length > 0) {
		labResults.update(($labs) =>
			$labs.filter((l) => !payload.removed_lab_ids!.includes(l.id))
		);
	}

	// Update timeline
	if (payload.timeline && payload.timeline.length > 0) {
		timelineEvents.update(($events) => {
			const updated = [...$events];
			for (const newEvent of payload.timeline!) {
				const idx = updated.findIndex((e) => e.id === newEvent.id);
				if (idx >= 0) updated[idx] = newEvent;
				else updated.push(newEvent);
			}
			return updated;
		});
	}
	if (payload.removed_timeline_ids && payload.removed_timeline_ids.length > 0) {
		timelineEvents.update(($events) =>
			$events.filter((e) => !payload.removed_timeline_ids!.includes(e.id))
		);
	}

	// Update alerts
	if (payload.alerts && payload.alerts.length > 0) {
		activeAlerts.update(($alerts) => {
			const updated = [...$alerts];
			for (const newAlert of payload.alerts!) {
				const idx = updated.findIndex((a) => a.id === newAlert.id);
				if (idx >= 0) updated[idx] = newAlert;
				else updated.push(newAlert);
			}
			return updated;
		});
	}
	if (payload.removed_alert_ids && payload.removed_alert_ids.length > 0) {
		activeAlerts.update(($alerts) =>
			$alerts.filter((a) => !payload.removed_alert_ids!.includes(a.id))
		);
	}

	// Update appointment (replace or remove)
	if (payload.appointment !== undefined) {
		nextAppointment.set(payload.appointment);
	}

	// Update profile (RS-M1-06-003: preserve emergency contacts)
	if (payload.profile) {
		profile.set({
			name: payload.profile.name,
			bloodType: payload.profile.blood_type,
			allergies: payload.profile.allergies,
			emergencyContacts: payload.profile.emergency_contacts.map((c) => ({
				name: c.name,
				phone: c.phone,
				relation: c.relation
			}))
		});
	}

	// Update sync state
	lastSyncTimestamp.set(payload.synced_at);
	syncState.update(($s) => ({
		...$s,
		versions: payload.versions,
		lastSyncAt: payload.synced_at
	}));
}

// === EXPIRED APPOINTMENT CLEANUP ===

/** Remove cached appointment if its date is in the past */
export function cleanupExpiredAppointment(now?: Date): boolean {
	const appt = get(nextAppointment);
	if (!appt) return false;

	const apptDate = new Date(appt.date);
	const current = now ?? new Date();

	if (apptDate.getTime() < current.getTime()) {
		nextAppointment.set(null);
		return true;
	}
	return false;
}

// === DEFERRED QUESTIONS ===

let questionCounter = 0;

export function saveDeferredQuestion(questionText: string): DeferredQuestion {
	const question: DeferredQuestion = {
		id: `deferred-${Date.now()}-${++questionCounter}`,
		questionText,
		createdAt: new Date().toISOString(),
		asked: false,
		askedAt: null
	};
	deferredQuestions.update(($q) => [...$q, question]);
	return question;
}

export function markQuestionsAsked(ids: string[]): void {
	const now = new Date().toISOString();
	deferredQuestions.update(($q) =>
		$q.map((q) => {
			if (!ids.includes(q.id)) return q;
			return { ...q, asked: true, askedAt: now };
		})
	);
}

export function clearDeferredQuestions(ids: string[]): void {
	deferredQuestions.update(($q) =>
		$q.filter((q) => !ids.includes(q.id))
	);
}

export function getPendingQuestions(): DeferredQuestion[] {
	return get(deferredQuestions).filter((q) => !q.asked);
}

// === WIPE PROTOCOL ===

/** Execute wipe protocol — full or health-data only */
export function wipeCache(scope: WipeScope): void {
	// Always clear health data stores
	clearCacheStores();

	// Clear deferred questions (health-related)
	deferredQuestions.set([]);

	// Reset sync state
	syncState.set({
		versions: emptySyncVersions(),
		lastSyncAt: null,
		cachePopulated: false
	});

	// Reset first sync progress
	firstSyncProgress.set(null);

	if (scope === 'full') {
		// In production: also clear Keychain entries (session_token,
		// cache_encryption_key, cert_fingerprint, device_id) and
		// clear app preferences (desktop_url, desktop_name, etc.)
		// keeping only biometric_enabled and slm_downloaded.
		// Here we just reset the counter.
		questionCounter = 0;
	}
}

/** Verify cache is fully wiped (post-wipe verification) */
export function verifyCacheWiped(): boolean {
	return (
		get(medications).length === 0 &&
		get(labResults).length === 0 &&
		get(timelineEvents).length === 0 &&
		get(activeAlerts).length === 0 &&
		get(nextAppointment) === null &&
		get(profile) === null &&
		get(lastSyncTimestamp) === null &&
		get(deferredQuestions).length === 0 &&
		!get(syncState).cachePopulated
	);
}

// === FIRST SYNC PROGRESS ===

export function setFirstSyncStage(stage: FirstSyncStage, percent: number): void {
	firstSyncProgress.set({ stage, percent });
}

export function clearFirstSyncProgress(): void {
	firstSyncProgress.set(null);
}

// === RESET (for tests) ===

export function resetCacheManagerState(): void {
	questionCounter = 0;
	wipeCache('full');
}
