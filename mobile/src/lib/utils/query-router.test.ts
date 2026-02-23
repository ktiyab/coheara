// M2-02: Query Router tests — safety blocklist, cache matching, routing logic, chips
// CA-08: Mock factories aligned with desktop source of truth (viewer.ts CA-05)
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
	checkSafety,
	matchQueryToCache,
	findTabRedirect,
	routeQuery,
	routeQuickQuestion,
	getQuickQuestionChips
} from './query-router.js';
import { connection, setConnected, setOffline, setUnpaired } from '$lib/stores/connection.js';
import { modelState, resetSlmState, startDownload, completeDownload, loadModel } from '$lib/stores/slm.js';
import { medications, labResults, timelineEvents, activeAlerts, nextAppointment } from '$lib/stores/cache.js';
import { applySyncPayload, resetCacheManagerState } from '$lib/stores/cache-manager.js';
import type { QuickQuestion } from '$lib/types/query-router.js';
import { QUICK_QUESTION_CHIPS } from '$lib/types/query-router.js';
import type { CachedMedication, CachedLabResult, CachedTimelineEvent, CachedAlert, CachedAppointment } from '$lib/types/viewer.js';

// === TEST HELPERS (aligned CA-05 desktop types) ===

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

function populateCache(): void {
	applySyncPayload({
		profile: { profileName: 'Thomas', totalDocuments: 5, extractionAccuracy: 0.88, allergies: [{ allergen: 'Penicillin', severity: 'high', verified: true }] },
		medications: [makeMed()],
		labs: [makeLab()],
		timeline: [makeEvent()],
		alerts: [makeAlert()],
		appointment: { id: 'appt-1', professionalName: 'Dr. Chen', date: '2026-03-01T10:00:00Z', appointmentType: 'Follow-up', prepAvailable: true },
		versions: { medications: 1, labs: 1, timeline: 1, alerts: 1, appointments: 1, profile: 1 },
		syncedAt: new Date().toISOString()
	});
}

function setupSlmReady(): void {
	resetSlmState();
	startDownload('gemma-2b-q4');
	completeDownload();
	loadModel();
}

beforeEach(() => {
	resetCacheManagerState();
	resetSlmState();
	setUnpaired();
});

// === SAFETY BLOCKLIST ===

describe('query-router — safety blocklist', () => {
	it('blocks drug interaction queries (desktop allowed)', () => {
		const result = checkSafety('Any drug interactions?');
		expect(result.blocked).toBe(true);
		expect(result.desktopAllowed).toBe(true);
		expect(result.category).toBe('drug_interactions');
	});

	it('blocks dosage change queries (doctor only)', () => {
		const result = checkSafety('Should I change my dose?');
		expect(result.blocked).toBe(true);
		expect(result.desktopAllowed).toBe(false);
		expect(result.category).toBe('dosage_change');
	});

	it('blocks symptom assessment (doctor only)', () => {
		const result = checkSafety('Is it serious?');
		expect(result.blocked).toBe(true);
		expect(result.desktopAllowed).toBe(false);
		expect(result.category).toBe('symptom_assessment');
	});

	it('blocks side effect queries (desktop allowed)', () => {
		const result = checkSafety('Is this a side effect of my medication?');
		expect(result.blocked).toBe(true);
		expect(result.desktopAllowed).toBe(true);
		expect(result.category).toBe('side_effects');
	});

	it('blocks treatment queries (doctor only)', () => {
		const result = checkSafety('What treatment should I try?');
		expect(result.blocked).toBe(true);
		expect(result.desktopAllowed).toBe(false);
		expect(result.category).toBe('treatment_advice');
	});

	it('blocks emergency queries (emergency services)', () => {
		const result = checkSafety("I can't breathe");
		expect(result.blocked).toBe(true);
		expect(result.desktopAllowed).toBe(false);
		expect(result.category).toBe('emergency');
		expect(result.userMessage).toContain('emergency services');
	});

	it('passes medication recall queries', () => {
		expect(checkSafety('What medications am I taking?').blocked).toBe(false);
	});

	it('passes lab recall queries', () => {
		expect(checkSafety('What was my HbA1c?').blocked).toBe(false);
	});

	it('passes appointment recall queries', () => {
		expect(checkSafety('When is my next appointment?').blocked).toBe(false);
	});

	it('blocks case-insensitively', () => {
		expect(checkSafety('DRUG INTERACTION check').blocked).toBe(true);
		expect(checkSafety('Should I STOP TAKING my meds?').blocked).toBe(true);
	});

	it('blocks "stop taking" pattern', () => {
		const result = checkSafety('Should I stop taking my blood thinner before surgery?');
		expect(result.blocked).toBe(true);
		expect(result.category).toBe('dosage_change');
	});

	it('blocks "missed dose" pattern', () => {
		expect(checkSafety('I missed a dose of my medication').blocked).toBe(true);
	});

	it('blocks chest pain as emergency', () => {
		expect(checkSafety('I have chest pain').blocked).toBe(true);
		expect(checkSafety('I have chest pain').category).toBe('emergency');
	});

	it('blocks overdose query', () => {
		expect(checkSafety('Did I overdose?').blocked).toBe(true);
		expect(checkSafety('Did I overdose?').category).toBe('emergency');
	});
});

// === CACHE MATCHING ===

describe('query-router — cache matching', () => {
	beforeEach(() => populateCache());

	it('matches medication keywords with high confidence', () => {
		const result = matchQueryToCache('What medications am I taking?');
		expect(result.scope.medications).toBe(true);
		expect(result.confidence).toBeGreaterThanOrEqual(0.7);
	});

	it('matches lab keywords', () => {
		const result = matchQueryToCache('Latest blood test results');
		expect(result.scope.labs).toBe(true);
		expect(result.confidence).toBeGreaterThanOrEqual(0.7);
	});

	it('matches timeline keywords', () => {
		const result = matchQueryToCache('When did I start my medication recently?');
		expect(result.scope.timeline).toBe(true);
	});

	it('matches appointment keywords', () => {
		const result = matchQueryToCache('When is my next doctor visit?');
		expect(result.scope.appointment).toBe(true);
	});

	it('matches multi-scope queries', () => {
		const result = matchQueryToCache('Are my medications affecting my blood test results?');
		expect(result.scope.medications).toBe(true);
		expect(result.scope.labs).toBe(true);
		expect(result.confidence).toBeGreaterThanOrEqual(0.85);
	});

	it('returns low confidence for no keyword match', () => {
		const result = matchQueryToCache('Tell me about myself');
		expect(result.confidence).toBe(0.3);
		expect(result.scope.medications).toBe(true); // All sections included at low confidence
	});

	it('returns zero confidence with empty cache', () => {
		resetCacheManagerState();
		const result = matchQueryToCache('Tell me something');
		expect(result.confidence).toBe(0.0);
	});

	it('returns low confidence when keywords match but no data', () => {
		medications.set([]);
		const result = matchQueryToCache('What medication am I on?');
		expect(result.confidence).toBe(0.1);
	});

	it('always includes profile in scope', () => {
		const result = matchQueryToCache('What medications am I taking?');
		expect(result.scope.profile).toBe(true);
	});
});

// === TAB REDIRECT ===

describe('query-router — tab redirect', () => {
	it('redirects medication queries to medications tab', () => {
		expect(findTabRedirect('What medicine do I take?')).toBe('medications');
	});

	it('redirects lab queries to labs tab', () => {
		expect(findTabRedirect('Show me my lab results')).toBe('labs');
	});

	it('redirects appointment queries to appointments tab', () => {
		expect(findTabRedirect('When is my doctor appointment?')).toBe('appointments');
	});

	it('returns null for unrecognized queries', () => {
		expect(findTabRedirect('Hello there')).toBeNull();
	});
});

// === ROUTING LOGIC ===

describe('query-router — routing', () => {
	it('safety-blocked + connected routes to desktop', () => {
		setConnected('Thomas', '2026-02-12');
		const route = routeQuery('Any drug interactions?');
		expect(route.target).toBe('desktop');
	});

	it('safety-blocked + disconnected shows message', () => {
		setUnpaired();
		const route = routeQuery('Any drug interactions?');
		expect(route.target).toBe('safety_blocked');
		if (route.target === 'safety_blocked') {
			expect(route.message).toContain('desktop');
		}
	});

	it('safety-blocked doctor-only + connected still shows message', () => {
		setConnected('Thomas', '2026-02-12');
		const route = routeQuery('Should I change my dose?');
		expect(route.target).toBe('safety_blocked');
	});

	it('connected always routes to desktop for safe queries', () => {
		setConnected('Thomas', '2026-02-12');
		const route = routeQuery('What are my medications?');
		expect(route.target).toBe('desktop');
	});

	it('disconnected + SLM ready + high confidence routes SLM', () => {
		setupSlmReady();
		populateCache();
		// Multi-keyword match → confidence >= 0.85 → high
		const route = routeQuery('What are my prescribed medication test results?');
		expect(route.target).toBe('slm');
		if (route.target === 'slm') {
			expect(route.confidence).toBe('high');
		}
	});

	it('disconnected + SLM ready + low confidence routes SLM with caveat', () => {
		setupSlmReady();
		populateCache();
		const route = routeQuery('Tell me something general');
		// 0.3 confidence (populated cache, no keywords) → below 0.4 → fallback
		expect(route.target).toBe('deferred');
	});

	it('disconnected + no SLM redirects to tab', () => {
		const route = routeQuery('What medication do I take?');
		expect(route.target).toBe('fallback_tab');
		if (route.target === 'fallback_tab') {
			expect(route.tab).toBe('medications');
		}
	});

	it('disconnected + no SLM + no tab match defers', () => {
		const route = routeQuery('Hello there');
		expect(route.target).toBe('deferred');
		if (route.target === 'deferred') {
			expect(route.query).toBe('Hello there');
		}
	});
});

// === QUICK-QUESTION CHIPS ===

describe('query-router — quick-question chips', () => {
	it('pre-classified chip skips keyword matching when SLM ready', () => {
		setupSlmReady();
		const chip = QUICK_QUESTION_CHIPS.find(c => c.label === 'My medications')!;
		const route = routeQuickQuestion(chip);
		expect(route.target).toBe('slm');
		if (route.target === 'slm') {
			expect(route.confidence).toBe('high');
			expect(route.cacheScope.medications).toBe(true);
		}
	});

	it('pre-classified chip routes to desktop when connected', () => {
		setConnected('Thomas', '2026-02-12');
		const chip = QUICK_QUESTION_CHIPS.find(c => c.label === 'My medications')!;
		const route = routeQuickQuestion(chip);
		expect(route.target).toBe('desktop');
	});

	it('non-SLM-capable chip always goes to desktop when connected', () => {
		setConnected('Thomas', '2026-02-12');
		const chip = QUICK_QUESTION_CHIPS.find(c => c.label === 'What to ask my doctor')!;
		expect(chip.slmCapable).toBe(false);
		const route = routeQuickQuestion(chip);
		expect(route.target).toBe('desktop');
	});

	it('non-SLM-capable chip defers when disconnected', () => {
		const chip = QUICK_QUESTION_CHIPS.find(c => c.label === 'What to ask my doctor')!;
		const route = routeQuickQuestion(chip);
		expect(route.target).toBe('deferred');
	});

	it('provides all 5 quick-question chips', () => {
		const chips = getQuickQuestionChips();
		expect(chips).toHaveLength(5);
		expect(chips.map(c => c.label)).toContain('My medications');
		expect(chips.map(c => c.label)).toContain('Next appointment');
		expect(chips.map(c => c.label)).toContain('Recent lab results');
		expect(chips.map(c => c.label)).toContain('What to ask my doctor');
		expect(chips.map(c => c.label)).toContain('Active alerts');
	});

	it('chip falls back to tab redirect when no SLM', () => {
		const chip = QUICK_QUESTION_CHIPS.find(c => c.label === 'My medications')!;
		const route = routeQuickQuestion(chip);
		expect(route.target).toBe('fallback_tab');
		if (route.target === 'fallback_tab') {
			expect(route.tab).toBe('medications');
		}
	});
});
