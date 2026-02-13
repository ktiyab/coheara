// M2-02: Query Router — safety blocklist, cache matching, routing logic
import { get } from 'svelte/store';
import type {
	QueryRoute,
	SafetyCheckResult,
	SafetyCategory,
	CacheMatchResult,
	QuickQuestion
} from '$lib/types/query-router.js';
import {
	SAFETY_MESSAGES,
	SAFETY_REASONS,
	DESKTOP_ALLOWED_CATEGORIES,
	INTERACTION_PATTERNS,
	DOSAGE_CHANGE_PATTERNS,
	SYMPTOM_PATTERNS,
	SIDE_EFFECT_PATTERNS,
	TREATMENT_PATTERNS,
	EMERGENCY_PATTERNS,
	MEDICATION_KEYWORDS,
	LAB_KEYWORDS,
	TIMELINE_KEYWORDS,
	APPOINTMENT_KEYWORDS,
	ALERT_KEYWORDS,
	QUICK_QUESTION_CHIPS
} from '$lib/types/query-router.js';
import type { CacheScope } from '$lib/types/slm.js';
import { isConnected } from '$lib/stores/connection.js';
import { isModelReady, isModelDownloaded } from '$lib/stores/slm.js';
import { cachePopulated } from '$lib/stores/cache-manager.js';
import { medications, labResults, timelineEvents, activeAlerts, nextAppointment } from '$lib/stores/cache.js';

// === SAFETY BLOCKLIST ===

/** Check if a query touches safety-blocked topics */
export function checkSafety(query: string): SafetyCheckResult {
	const lower = query.toLowerCase();

	const checks: Array<{ patterns: RegExp[]; category: SafetyCategory }> = [
		{ patterns: INTERACTION_PATTERNS, category: 'drug_interactions' },
		{ patterns: DOSAGE_CHANGE_PATTERNS, category: 'dosage_change' },
		{ patterns: SYMPTOM_PATTERNS, category: 'symptom_assessment' },
		{ patterns: SIDE_EFFECT_PATTERNS, category: 'side_effects' },
		{ patterns: TREATMENT_PATTERNS, category: 'treatment_advice' },
		{ patterns: EMERGENCY_PATTERNS, category: 'emergency' },
	];

	for (const { patterns, category } of checks) {
		if (patterns.some(p => p.test(lower))) {
			return {
				blocked: true,
				desktopAllowed: DESKTOP_ALLOWED_CATEGORIES.includes(category),
				category,
				reason: SAFETY_REASONS[category],
				userMessage: SAFETY_MESSAGES[category]
			};
		}
	}

	return { blocked: false };
}

// === CACHE MATCHING ===

/** Match a query to relevant cache sections with confidence score */
export function matchQueryToCache(query: string): CacheMatchResult {
	const lower = query.toLowerCase();
	const scope: CacheScope = {
		medications: false,
		labs: false,
		timeline: false,
		alerts: false,
		appointment: false,
		profile: false
	};

	let matchCount = 0;

	if (MEDICATION_KEYWORDS.some(kw => lower.includes(kw))) {
		scope.medications = true;
		matchCount++;
	}
	if (LAB_KEYWORDS.some(kw => lower.includes(kw))) {
		scope.labs = true;
		matchCount++;
	}
	if (TIMELINE_KEYWORDS.some(kw => lower.includes(kw))) {
		scope.timeline = true;
		matchCount++;
	}
	if (APPOINTMENT_KEYWORDS.some(kw => lower.includes(kw))) {
		scope.appointment = true;
		matchCount++;
	}
	if (ALERT_KEYWORDS.some(kw => lower.includes(kw))) {
		scope.alerts = true;
		matchCount++;
	}

	// Always include profile for context
	scope.profile = true;

	if (matchCount === 0) {
		const populated = get(cachePopulated);
		if (populated) {
			return {
				scope: { medications: true, labs: true, timeline: true, alerts: true, appointment: true, profile: true },
				confidence: 0.3
			};
		}
		return { scope, confidence: 0.0 };
	}

	// Verify matched sections have data (refine scope to data-backed sections)
	const refined = refineScope(scope);
	if (!refined) {
		return { scope, confidence: 0.1 };
	}

	const confidence = matchCount >= 3 ? 0.95
		: matchCount >= 2 ? 0.85
		: 0.7;

	return { scope: refined, confidence };
}

/** Refine scope to only sections that have cached data. Returns null if no data. */
function refineScope(scope: CacheScope): CacheScope | null {
	const out: CacheScope = { ...scope };
	if (out.medications && get(medications).length === 0) out.medications = false;
	if (out.labs && get(labResults).length === 0) out.labs = false;
	if (out.timeline && get(timelineEvents).length === 0) out.timeline = false;
	if (out.alerts && get(activeAlerts).length === 0) out.alerts = false;
	if (out.appointment && get(nextAppointment) === null) out.appointment = false;

	const hasData = out.medications || out.labs || out.timeline || out.alerts || out.appointment;
	return hasData ? out : null;
}

// === TAB REDIRECT ===

/** Find a tab that can directly show the requested data */
export function findTabRedirect(query: string): string | null {
	const lower = query.toLowerCase();
	if (MEDICATION_KEYWORDS.some(k => lower.includes(k))) return 'medications';
	if (LAB_KEYWORDS.some(k => lower.includes(k))) return 'labs';
	if (APPOINTMENT_KEYWORDS.some(k => lower.includes(k))) return 'appointments';
	return null;
}

// === MAIN ROUTER ===

/** Route a user query to the optimal answer source */
export function routeQuery(query: string): QueryRoute {
	// Step 0: Safety blocklist (ALWAYS first)
	const safety = checkSafety(query);
	if (safety.blocked) {
		// Safety-blocked but desktop can handle some categories
		if (get(isConnected) && safety.desktopAllowed) {
			return { target: 'desktop' };
		}
		return {
			target: 'safety_blocked',
			reason: safety.reason!,
			message: safety.userMessage!
		};
	}

	// Step 1: Desktop available? (best quality, always preferred)
	if (get(isConnected)) {
		return { target: 'desktop' };
	}

	// Step 2: SLM available + cache match?
	if (get(isModelReady) || get(isModelDownloaded)) {
		const cacheMatch = matchQueryToCache(query);

		if (cacheMatch.confidence >= 0.8) {
			return {
				target: 'slm',
				cacheScope: cacheMatch.scope,
				confidence: 'high'
			};
		}

		if (cacheMatch.confidence >= 0.4) {
			return {
				target: 'slm',
				cacheScope: cacheMatch.scope,
				confidence: 'low'
			};
		}
	}

	// Step 3: Tab redirect?
	const tab = findTabRedirect(query);
	if (tab) {
		return { target: 'fallback_tab', tab };
	}

	// Step 4: Defer
	return { target: 'deferred', query };
}

/** Route a pre-classified quick-question chip */
export function routeQuickQuestion(chip: QuickQuestion): QueryRoute {
	// Step 0: Non-SLM-capable chips always go to desktop
	if (!chip.slmCapable) {
		if (get(isConnected)) {
			return { target: 'desktop' };
		}
		return { target: 'deferred', query: chip.query };
	}

	// Step 1: Desktop available?
	if (get(isConnected)) {
		return { target: 'desktop' };
	}

	// Step 2: Pre-classified chips skip keyword matching
	if (chip.preClassified && (get(isModelReady) || get(isModelDownloaded))) {
		return {
			target: 'slm',
			cacheScope: chip.preClassified.scope,
			confidence: chip.preClassified.confidence >= 0.8 ? 'high' : 'low'
		};
	}

	// Step 3: Fallback
	const tab = findTabRedirect(chip.query);
	if (tab) {
		return { target: 'fallback_tab', tab };
	}

	return { target: 'deferred', query: chip.query };
}

/** Get all available quick-question chips */
export function getQuickQuestionChips(): QuickQuestion[] {
	return QUICK_QUESTION_CHIPS;
}
