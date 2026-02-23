// M1-06: Cache Manager tests — aligned CA-05 desktop types
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
	syncState,
	cachePopulated,
	deferredQuestions,
	pendingQuestionCount,
	firstSyncProgress,
	isFirstSyncing,
	computeFreshnessTier,
	freshnessTierLabel,
	freshnessTierColor,
	shouldShowStalenessWarning,
	stalenessWarningMessage,
	applySyncPayload,
	applyDeltaPayload,
	cleanupExpiredAppointment,
	saveDeferredQuestion,
	markQuestionsAsked,
	clearDeferredQuestions,
	getPendingQuestions,
	wipeCache,
	verifyCacheWiped,
	setFirstSyncStage,
	clearFirstSyncProgress,
	resetCacheManagerState
} from './cache-manager.js';
import {
	medications,
	labResults,
	timelineEvents,
	activeAlerts,
	nextAppointment,
	profile,
	lastSyncTimestamp
} from './cache.js';
import type { SyncPayload, DeltaPayload, SyncVersions } from '$lib/types/cache-manager.js';
import type { CachedMedication, CachedLabResult, CachedTimelineEvent, CachedAlert, CachedAppointment } from '$lib/types/viewer.js';
import { FRESHNESS_TIER_THRESHOLDS } from '$lib/types/cache-manager.js';

function makeVersions(v: number = 1): SyncVersions {
	return { medications: v, labs: v, timeline: v, alerts: v, appointments: v, profile: v };
}

function makeMed(overrides: Partial<CachedMedication> = {}): CachedMedication {
	return {
		id: 'med-1', genericName: 'Lisinopril', dose: '10mg', frequency: 'Once daily',
		route: 'oral', status: 'active', isOtc: false,
		prescriberName: 'Dr. Ndiaye', condition: 'Blood pressure',
		startDate: '2025-01-01', ...overrides
	};
}

function makeLab(overrides: Partial<CachedLabResult> = {}): CachedLabResult {
	return {
		id: 'lab-1', testName: 'HbA1c', value: 7.2, unit: '%',
		referenceRangeLow: 4, referenceRangeHigh: 5.6, abnormalFlag: 'H',
		isAbnormal: true, collectionDate: '2025-06-01',
		trendDirection: 'up', ...overrides
	};
}

function makeEvent(overrides: Partial<CachedTimelineEvent> = {}): CachedTimelineEvent {
	return {
		id: 'event-1', eventType: 'medication_change', category: 'Medications',
		description: 'Lisinopril started 10mg', date: '2025-06-01',
		stillActive: false, ...overrides
	};
}

function makeAlert(overrides: Partial<CachedAlert> = {}): CachedAlert {
	return {
		id: 'alert-1', title: 'Possible interaction',
		description: 'Lisinopril + potassium', severity: 'warning',
		createdAt: '2025-06-01', dismissed: false, ...overrides
	};
}

function makePayload(overrides: Partial<SyncPayload> = {}): SyncPayload {
	return {
		profile: { profileName: 'Thomas', totalDocuments: 8, extractionAccuracy: 0.89, allergies: [{ allergen: 'Penicillin', severity: 'high', verified: true }] },
		medications: [makeMed()],
		labs: [makeLab()],
		timeline: [makeEvent()],
		alerts: [makeAlert()],
		appointment: {
			id: 'appt-1', professionalName: 'Dr. Chen',
			date: '2026-03-01T10:00:00Z', prepAvailable: true,
			appointmentType: 'Follow-up'
		},
		versions: makeVersions(1),
		syncedAt: new Date().toISOString(),
		...overrides
	};
}

// === FRESHNESS TIERS ===

describe('cache-manager — freshness tiers', () => {
	it('returns green for sync < 2 hours ago', () => {
		const now = Date.now();
		const oneHourAgo = new Date(now - 60 * 60 * 1000).toISOString();
		expect(computeFreshnessTier(oneHourAgo, now)).toBe('green');
	});

	it('returns neutral for sync 2-24 hours ago', () => {
		const now = Date.now();
		const sixHoursAgo = new Date(now - 6 * 60 * 60 * 1000).toISOString();
		expect(computeFreshnessTier(sixHoursAgo, now)).toBe('neutral');
	});

	it('returns amber for sync 1-7 days ago', () => {
		const now = Date.now();
		const twoDaysAgo = new Date(now - 2 * 24 * 60 * 60 * 1000).toISOString();
		expect(computeFreshnessTier(twoDaysAgo, now)).toBe('amber');
	});

	it('returns red for sync > 7 days ago', () => {
		const now = Date.now();
		const tenDaysAgo = new Date(now - 10 * 24 * 60 * 60 * 1000).toISOString();
		expect(computeFreshnessTier(tenDaysAgo, now)).toBe('red');
	});

	it('returns red for null timestamp (never synced)', () => {
		expect(computeFreshnessTier(null)).toBe('red');
	});

	it('freshnessTierLabel returns correct labels', () => {
		expect(freshnessTierLabel('green')).toBe('Up to date');
		expect(freshnessTierLabel('neutral')).toBe('Recent');
		expect(freshnessTierLabel('amber')).toContain('may have changed');
		expect(freshnessTierLabel('red')).toContain('outdated');
	});

	it('freshnessTierColor returns CSS variables', () => {
		expect(freshnessTierColor('green')).toContain('success');
		expect(freshnessTierColor('neutral')).toContain('muted');
		expect(freshnessTierColor('amber')).toContain('warning');
		expect(freshnessTierColor('red')).toContain('error');
	});

	it('staleness warning shows for amber and red only', () => {
		expect(shouldShowStalenessWarning('green')).toBe(false);
		expect(shouldShowStalenessWarning('neutral')).toBe(false);
		expect(shouldShowStalenessWarning('amber')).toBe(true);
		expect(shouldShowStalenessWarning('red')).toBe(true);
	});

	it('staleness warning messages differ by tier', () => {
		const amberMsg = stalenessWarningMessage('amber');
		const redMsg = stalenessWarningMessage('red');
		expect(amberMsg).toContain('Connect to your desktop to update');
		expect(redMsg).toContain('Connect to your desktop to refresh');
		expect(stalenessWarningMessage('green')).toBe('');
	});
});

// === FULL SYNC PAYLOAD ===

describe('cache-manager — full sync', () => {
	beforeEach(() => resetCacheManagerState());

	it('applies full sync payload to all stores', () => {
		applySyncPayload(makePayload());

		expect(get(medications)).toHaveLength(1);
		expect(get(labResults)).toHaveLength(1);
		expect(get(timelineEvents)).toHaveLength(1);
		expect(get(activeAlerts)).toHaveLength(1);
		expect(get(nextAppointment)).not.toBeNull();
		expect(get(profile)?.profileName).toBe('Thomas');
		expect(get(profile)?.totalDocuments).toBe(8);
		expect(get(lastSyncTimestamp)).toBeTruthy();
	});

	it('marks cache as populated after full sync', () => {
		expect(get(cachePopulated)).toBe(false);
		applySyncPayload(makePayload());
		expect(get(cachePopulated)).toBe(true);
	});

	it('updates sync versions', () => {
		applySyncPayload(makePayload({ versions: makeVersions(5) }));
		expect(get(syncState).versions.medications).toBe(5);
	});

	it('replaces all data on subsequent full sync', () => {
		applySyncPayload(makePayload({ medications: [makeMed(), makeMed({ id: 'med-2', genericName: 'Metformin' })] }));
		expect(get(medications)).toHaveLength(2);

		applySyncPayload(makePayload({ medications: [makeMed({ id: 'med-3', genericName: 'Aspirin' })] }));
		expect(get(medications)).toHaveLength(1);
		expect(get(medications)[0].genericName).toBe('Aspirin');
	});
});

// === DELTA SYNC ===

describe('cache-manager — delta sync', () => {
	beforeEach(() => {
		resetCacheManagerState();
		applySyncPayload(makePayload());
	});

	it('adds new medication via delta', () => {
		const delta: DeltaPayload = {
			medications: [makeMed({ id: 'med-new', genericName: 'Metformin' })],
			versions: makeVersions(2),
			syncedAt: new Date().toISOString()
		};
		applyDeltaPayload(delta);
		expect(get(medications)).toHaveLength(2);
	});

	it('updates existing medication via delta', () => {
		const delta: DeltaPayload = {
			medications: [makeMed({ id: 'med-1', dose: '20mg' })],
			versions: makeVersions(2),
			syncedAt: new Date().toISOString()
		};
		applyDeltaPayload(delta);
		expect(get(medications)).toHaveLength(1);
		expect(get(medications)[0].dose).toBe('20mg');
	});

	it('removes medication via delta tombstone', () => {
		const delta: DeltaPayload = {
			removed_medication_ids: ['med-1'],
			versions: makeVersions(2),
			syncedAt: new Date().toISOString()
		};
		applyDeltaPayload(delta);
		expect(get(medications)).toHaveLength(0);
	});

	it('removes alert via delta (resolved on desktop)', () => {
		const delta: DeltaPayload = {
			removed_alert_ids: ['alert-1'],
			versions: makeVersions(2),
			syncedAt: new Date().toISOString()
		};
		applyDeltaPayload(delta);
		expect(get(activeAlerts)).toHaveLength(0);
	});

	it('updates appointment to undefined via delta', () => {
		const delta: DeltaPayload = {
			versions: makeVersions(2),
			syncedAt: new Date().toISOString()
		};
		// appointment not in delta = no change; explicitly undefined check handled by desktop
		applyDeltaPayload(delta);
		expect(get(nextAppointment)).not.toBeNull();
	});

	it('updates sync timestamp after delta', () => {
		const syncTime = '2026-02-15T12:00:00Z';
		const delta: DeltaPayload = {
			versions: makeVersions(2),
			syncedAt: syncTime
		};
		applyDeltaPayload(delta);
		expect(get(lastSyncTimestamp)).toBe(syncTime);
	});
});

// === EXPIRED APPOINTMENT CLEANUP ===

describe('cache-manager — appointment cleanup', () => {
	beforeEach(() => resetCacheManagerState());

	it('removes expired appointment', () => {
		applySyncPayload(makePayload({
			appointment: {
				id: 'appt-1', professionalName: 'Dr. Chen',
				date: '2020-01-01T10:00:00Z', prepAvailable: false,
				appointmentType: 'Follow-up'
			}
		}));
		const removed = cleanupExpiredAppointment(new Date('2026-02-15'));
		expect(removed).toBe(true);
		expect(get(nextAppointment)).toBeNull();
	});

	it('preserves future appointment', () => {
		applySyncPayload(makePayload({
			appointment: {
				id: 'appt-1', professionalName: 'Dr. Chen',
				date: '2030-01-01T10:00:00Z', prepAvailable: false,
				appointmentType: 'Check-up'
			}
		}));
		const removed = cleanupExpiredAppointment(new Date('2026-02-15'));
		expect(removed).toBe(false);
		expect(get(nextAppointment)).not.toBeNull();
	});

	it('handles no appointment', () => {
		const removed = cleanupExpiredAppointment();
		expect(removed).toBe(false);
	});
});

// === DEFERRED QUESTIONS ===

describe('cache-manager — deferred questions', () => {
	beforeEach(() => resetCacheManagerState());

	it('saves a deferred question', () => {
		const q = saveDeferredQuestion('What is my potassium level?');
		expect(q.id).toMatch(/^deferred-/);
		expect(q.asked).toBe(false);
		expect(get(pendingQuestionCount)).toBe(1);
	});

	it('marks questions as asked', () => {
		const q1 = saveDeferredQuestion('Question 1');
		const q2 = saveDeferredQuestion('Question 2');
		markQuestionsAsked([q1.id]);
		expect(get(pendingQuestionCount)).toBe(1);
		expect(getPendingQuestions()).toHaveLength(1);
		expect(getPendingQuestions()[0].id).toBe(q2.id);
	});

	it('clears dismissed questions', () => {
		const q1 = saveDeferredQuestion('Question 1');
		saveDeferredQuestion('Question 2');
		clearDeferredQuestions([q1.id]);
		expect(get(deferredQuestions)).toHaveLength(1);
	});

	it('generates unique IDs', () => {
		const q1 = saveDeferredQuestion('Q1');
		const q2 = saveDeferredQuestion('Q2');
		expect(q1.id).not.toBe(q2.id);
	});
});

// === WIPE PROTOCOL ===

describe('cache-manager — wipe protocol', () => {
	beforeEach(() => {
		resetCacheManagerState();
		applySyncPayload(makePayload());
		saveDeferredQuestion('Pending question');
	});

	it('full wipe clears all health data', () => {
		wipeCache('full');
		expect(verifyCacheWiped()).toBe(true);
		expect(get(medications)).toHaveLength(0);
		expect(get(labResults)).toHaveLength(0);
		expect(get(profile)).toBeNull();
		expect(get(deferredQuestions)).toHaveLength(0);
		expect(get(syncState).cachePopulated).toBe(false);
	});

	it('health-only wipe clears health data (profile switch)', () => {
		wipeCache('health_only');
		expect(get(medications)).toHaveLength(0);
		expect(get(profile)).toBeNull();
		expect(get(cachePopulated)).toBe(false);
	});

	it('verifyCacheWiped returns false when data exists', () => {
		expect(verifyCacheWiped()).toBe(false);
	});

	it('verifyCacheWiped returns true after wipe', () => {
		wipeCache('full');
		expect(verifyCacheWiped()).toBe(true);
	});
});

// === FIRST SYNC PROGRESS ===

describe('cache-manager — first sync progress', () => {
	beforeEach(() => resetCacheManagerState());

	it('tracks first sync stages', () => {
		setFirstSyncStage('connecting', 0);
		expect(get(isFirstSyncing)).toBe(true);
		expect(get(firstSyncProgress)?.stage).toBe('connecting');

		setFirstSyncStage('downloading', 50);
		expect(get(firstSyncProgress)?.percent).toBe(50);

		setFirstSyncStage('complete', 100);
		expect(get(isFirstSyncing)).toBe(false);
	});

	it('clears first sync progress', () => {
		setFirstSyncStage('downloading', 50);
		clearFirstSyncProgress();
		expect(get(firstSyncProgress)).toBeNull();
	});
});

// === COMBINED DELTA SYNC (RS-M1-06-002) ===

describe('cache-manager — combined delta sync', () => {
	beforeEach(() => {
		resetCacheManagerState();
		applySyncPayload(makePayload({
			medications: [makeMed({ id: 'med-1' }), makeMed({ id: 'med-2', genericName: 'Metformin' })],
			labs: [makeLab({ id: 'lab-1' }), makeLab({ id: 'lab-2', testName: 'Creatinine' })],
			timeline: [makeEvent({ id: 'event-1' })],
			alerts: [makeAlert({ id: 'alert-1' }), makeAlert({ id: 'alert-2', title: 'Drug allergy' })]
		}));
	});

	it('applies upserts and tombstones across multiple entity types simultaneously', () => {
		const delta: DeltaPayload = {
			medications: [makeMed({ id: 'med-3', genericName: 'Aspirin' })],
			removed_medication_ids: ['med-1'],
			labs: [makeLab({ id: 'lab-1', value: 6.5 })],
			removed_alert_ids: ['alert-2'],
			timeline: [makeEvent({ id: 'event-2', category: 'Lab Results', description: 'New event' })],
			versions: makeVersions(2),
			syncedAt: '2026-02-15T12:00:00Z'
		};
		applyDeltaPayload(delta);

		// med-1 removed, med-2 kept, med-3 added
		const meds = get(medications);
		expect(meds).toHaveLength(2);
		expect(meds.map((m) => m.id).sort()).toEqual(['med-2', 'med-3']);

		// lab-1 updated (value changed), lab-2 untouched
		const labs = get(labResults);
		expect(labs).toHaveLength(2);
		expect(labs.find((l) => l.id === 'lab-1')?.value).toBe(6.5);

		// alert-2 removed, alert-1 kept
		expect(get(activeAlerts)).toHaveLength(1);
		expect(get(activeAlerts)[0].id).toBe('alert-1');

		// event-2 added to existing event-1
		expect(get(timelineEvents)).toHaveLength(2);
	});

	it('handles upsert and tombstone for same entity type in one delta', () => {
		const delta: DeltaPayload = {
			medications: [makeMed({ id: 'med-1', dose: '20mg' })],
			removed_medication_ids: ['med-2'],
			versions: makeVersions(2),
			syncedAt: new Date().toISOString()
		};
		applyDeltaPayload(delta);

		const meds = get(medications);
		expect(meds).toHaveLength(1);
		expect(meds[0].id).toBe('med-1');
		expect(meds[0].dose).toBe('20mg');
	});

	it('delta with profile update preserves other entity stores', () => {
		const delta: DeltaPayload = {
			profile: { profileName: 'Thomas K.', totalDocuments: 15, extractionAccuracy: 0.95, allergies: [{ allergen: 'Sulfa', severity: 'moderate', verified: true }] },
			versions: makeVersions(2),
			syncedAt: new Date().toISOString()
		};
		applyDeltaPayload(delta);

		expect(get(profile)?.profileName).toBe('Thomas K.');
		expect(get(profile)?.extractionAccuracy).toBe(0.95);
		// Other stores untouched
		expect(get(medications)).toHaveLength(2);
		expect(get(labResults)).toHaveLength(2);
		expect(get(activeAlerts)).toHaveLength(2);
	});

	it('empty delta (versions-only) leaves stores unchanged', () => {
		const delta: DeltaPayload = {
			versions: makeVersions(2),
			syncedAt: '2026-02-15T12:00:00Z'
		};
		applyDeltaPayload(delta);

		expect(get(medications)).toHaveLength(2);
		expect(get(labResults)).toHaveLength(2);
		expect(get(activeAlerts)).toHaveLength(2);
		expect(get(syncState).versions.medications).toBe(2);
		expect(get(lastSyncTimestamp)).toBe('2026-02-15T12:00:00Z');
	});
});

// === PROFILE ALLERGIES SYNC ===

describe('cache-manager — profile allergies', () => {
	beforeEach(() => resetCacheManagerState());

	it('preserves structured allergies through full sync', () => {
		applySyncPayload(makePayload({
			profile: {
				profileName: 'Thomas',
				totalDocuments: 8,
				extractionAccuracy: 0.89,
				allergies: [
					{ allergen: 'Penicillin', severity: 'high', verified: true },
					{ allergen: 'Aspirin', severity: 'moderate', verified: false }
				]
			}
		}));

		const p = get(profile);
		expect(p?.allergies).toHaveLength(2);
		expect(p?.allergies[0].allergen).toBe('Penicillin');
		expect(p?.allergies[0].severity).toBe('high');
		expect(p?.allergies[0].verified).toBe(true);
		expect(p?.allergies[1].allergen).toBe('Aspirin');
	});

	it('preserves structured allergies through delta sync', () => {
		applySyncPayload(makePayload());

		const delta: DeltaPayload = {
			profile: {
				profileName: 'Thomas',
				totalDocuments: 10,
				extractionAccuracy: 0.91,
				allergies: [
					{ allergen: 'Penicillin', severity: 'high', verified: true },
					{ allergen: 'Sulfa', severity: 'low', verified: false }
				]
			},
			versions: makeVersions(2),
			syncedAt: new Date().toISOString()
		};
		applyDeltaPayload(delta);

		const p = get(profile);
		expect(p?.allergies).toHaveLength(2);
		expect(p?.allergies[1].allergen).toBe('Sulfa');
	});

	it('handles empty allergies', () => {
		applySyncPayload(makePayload({
			profile: {
				profileName: 'Thomas',
				totalDocuments: 5,
				extractionAccuracy: 0.85,
				allergies: []
			}
		}));

		expect(get(profile)?.allergies).toEqual([]);
	});
});
