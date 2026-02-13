// M2-03: Safety Filter tests — regex patterns, grounding check, filter outcomes
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
	scanRegexPatterns,
	checkMedicationGrounding,
	checkLabGrounding,
	checkGrounding,
	canRephrase,
	rephraseResponse,
	filterResponse,
	extractMedicationMentions,
	extractLabMentions,
	normalizeDose,
	ALL_PATTERNS
} from './safety-filter.js';
import { BLOCKED_FALLBACK_MESSAGE } from '$lib/types/safety-filter.js';
import { medications, labResults, clearCacheStores } from '$lib/stores/cache.js';
import type { CachedMedication, CachedLabResult } from '$lib/types/viewer.js';

// === TEST HELPERS ===

function makeMed(overrides: Partial<CachedMedication> = {}): CachedMedication {
	return {
		id: 'med-1', name: 'Metformin', dose: '500mg', frequency: 'Twice daily',
		prescriber: 'Dr. Ndiaye', purpose: 'Blood sugar', scheduleGroup: 'morning',
		since: '2025-01-01', isActive: true, ...overrides
	};
}

function makeLab(overrides: Partial<CachedLabResult> = {}): CachedLabResult {
	return {
		id: 'lab-1', testName: 'HbA1c', value: 7.2, unit: '%',
		referenceMin: 4, referenceMax: 5.6, isAbnormal: true,
		trend: 'up', trendContext: 'worsening', testedAt: '2025-06-01',
		...overrides
	};
}

beforeEach(() => {
	clearCacheStores();
});

// === LAYER 2: DIAGNOSTIC PATTERNS ===

describe('safety-filter — diagnostic patterns', () => {
	it('catches "you have [condition]"', () => {
		const violations = scanRegexPatterns('you have diabetes');
		expect(violations.length).toBeGreaterThan(0);
		expect(violations[0].category).toBe('diagnostic');
	});

	it('catches "you are suffering from"', () => {
		const violations = scanRegexPatterns('you are suffering from chronic pain');
		expect(violations.length).toBeGreaterThan(0);
		expect(violations[0].category).toBe('diagnostic');
	});

	it('catches "you likely have"', () => {
		const violations = scanRegexPatterns('you likely have an infection');
		expect(violations.length).toBeGreaterThan(0);
		expect(violations[0].category).toBe('diagnostic');
	});

	it('catches "you\'re diabetic"', () => {
		const violations = scanRegexPatterns("you're diabetic");
		expect(violations.length).toBeGreaterThan(0);
		expect(violations[0].category).toBe('diagnostic');
	});

	it('catches "your condition is"', () => {
		const violations = scanRegexPatterns('your condition is worsening');
		expect(violations.length).toBeGreaterThan(0);
		expect(violations[0].category).toBe('diagnostic');
	});

	it('catches "you appear to have"', () => {
		const violations = scanRegexPatterns('you appear to have thyroid issues');
		expect(violations.length).toBeGreaterThan(0);
		expect(violations[0].category).toBe('diagnostic');
	});
});

// === LAYER 2: PRESCRIPTIVE PATTERNS ===

describe('safety-filter — prescriptive patterns', () => {
	it('catches "you should take"', () => {
		const violations = scanRegexPatterns('you should take aspirin daily');
		expect(violations.length).toBeGreaterThan(0);
		expect(violations[0].category).toBe('prescriptive');
	});

	it('catches "I recommend"', () => {
		const violations = scanRegexPatterns('I recommend starting a diet');
		expect(violations.length).toBeGreaterThan(0);
		expect(violations[0].category).toBe('prescriptive');
	});

	it('catches "you need to see"', () => {
		const violations = scanRegexPatterns('you need to see a specialist');
		expect(violations.length).toBeGreaterThan(0);
		expect(violations[0].category).toBe('prescriptive');
	});

	it('catches "do not take"', () => {
		const violations = scanRegexPatterns('do not take this with alcohol');
		expect(violations.length).toBeGreaterThan(0);
		expect(violations[0].category).toBe('prescriptive');
	});

	it('catches "try taking"', () => {
		const violations = scanRegexPatterns('try taking this in the morning');
		expect(violations.length).toBeGreaterThan(0);
		expect(violations[0].category).toBe('prescriptive');
	});

	it('catches "consider stopping"', () => {
		const violations = scanRegexPatterns('consider stopping before surgery');
		expect(violations.length).toBeGreaterThan(0);
		expect(violations[0].category).toBe('prescriptive');
	});
});

// === LAYER 2: ALARM PATTERNS ===

describe('safety-filter — alarm patterns', () => {
	it('catches "dangerous"', () => {
		const violations = scanRegexPatterns('this could be dangerous');
		expect(violations.length).toBeGreaterThan(0);
		expect(violations[0].category).toBe('alarm');
	});

	it('catches "immediately go"', () => {
		const violations = scanRegexPatterns('immediately go to the ER');
		expect(violations.length).toBeGreaterThan(0);
		expect(violations[0].category).toBe('alarm');
	});

	it('catches "call 911"', () => {
		const violations = scanRegexPatterns('call 911 right away');
		expect(violations.length).toBeGreaterThan(0);
		expect(violations[0].category).toBe('alarm');
	});

	it('catches "life-threatening"', () => {
		const violations = scanRegexPatterns('this could be life-threatening');
		expect(violations.length).toBeGreaterThan(0);
		expect(violations[0].category).toBe('alarm');
	});

	it('catches "do not wait"', () => {
		const violations = scanRegexPatterns('do not wait to seek treatment');
		expect(violations.length).toBeGreaterThan(0);
		expect(violations[0].category).toBe('alarm');
	});
});

// === CLEAN PASS ===

describe('safety-filter — clean pass', () => {
	it('passes clean response with proper framing', () => {
		const violations = scanRegexPatterns(
			'Your records show Metformin 500mg twice daily, prescribed by Dr. Chen.'
		);
		expect(violations).toHaveLength(0);
	});

	it('passes safe framing language', () => {
		const violations = scanRegexPatterns(
			'Consider discussing this with your healthcare team.'
		);
		expect(violations).toHaveLength(0);
	});

	it('detects case-insensitively', () => {
		const v1 = scanRegexPatterns('YOU HAVE diabetes');
		const v2 = scanRegexPatterns('you Have Diabetes');
		expect(v1.length).toBeGreaterThan(0);
		expect(v2.length).toBeGreaterThan(0);
	});

	it('has exactly 24 total patterns', () => {
		expect(ALL_PATTERNS).toHaveLength(24);
	});
});

// === GROUNDING CHECK ===

describe('safety-filter — medication grounding', () => {
	it('detects dose mismatch', () => {
		medications.set([makeMed({ name: 'Metformin', dose: '250mg' })]);
		const issues = checkMedicationGrounding('Your records show Metformin 500mg daily.');
		expect(issues.length).toBeGreaterThan(0);
		expect(issues[0].type).toBe('dose_mismatch');
		expect(issues[0].claimed).toBe('500mg');
		expect(issues[0].cached).toBe('250mg');
	});

	it('detects unknown medication', () => {
		medications.set([makeMed({ name: 'Metformin' })]);
		const issues = checkMedicationGrounding('You are taking Aspirin 100mg for pain.');
		expect(issues.length).toBeGreaterThan(0);
		expect(issues[0].type).toBe('unknown_medication');
		expect(issues[0].claimed).toBe('Aspirin');
	});

	it('passes matching dose', () => {
		medications.set([makeMed({ name: 'Metformin', dose: '500mg' })]);
		const issues = checkMedicationGrounding('Your records show Metformin 500mg daily.');
		expect(issues).toHaveLength(0);
	});

	it('returns empty for no cache data', () => {
		const issues = checkMedicationGrounding('Metformin 500mg daily');
		expect(issues).toHaveLength(0);
	});
});

describe('safety-filter — lab grounding', () => {
	it('detects lab value mismatch', () => {
		labResults.set([makeLab({ testName: 'HbA1c', value: 7.2 })]);
		const issues = checkLabGrounding('Your HbA1c is 8.1%');
		expect(issues.length).toBeGreaterThan(0);
		expect(issues[0].type).toBe('value_mismatch');
	});

	it('passes matching lab value', () => {
		labResults.set([makeLab({ testName: 'HbA1c', value: 7.2 })]);
		const issues = checkLabGrounding('Your HbA1c is 7.2%');
		expect(issues).toHaveLength(0);
	});

	it('returns empty for no cached labs', () => {
		const issues = checkLabGrounding('HbA1c 7.2');
		expect(issues).toHaveLength(0);
	});
});

// === REPHRASE / BLOCK DECISION ===

describe('safety-filter — rephrase/block decisions', () => {
	it('can rephrase 1-2 non-alarm violations', () => {
		const violations = [
			{ category: 'prescriptive' as const, matchedText: 'you should take', pattern: 'test', offset: 0 }
		];
		expect(canRephrase(violations, [])).toBe(true);
	});

	it('cannot rephrase alarm violations', () => {
		const violations = [
			{ category: 'alarm' as const, matchedText: 'dangerous', pattern: 'test', offset: 0 }
		];
		expect(canRephrase(violations, [])).toBe(false);
	});

	it('cannot rephrase 3+ violations', () => {
		const violations = [
			{ category: 'prescriptive' as const, matchedText: 'a', pattern: 'test', offset: 0 },
			{ category: 'diagnostic' as const, matchedText: 'b', pattern: 'test', offset: 10 },
			{ category: 'prescriptive' as const, matchedText: 'c', pattern: 'test', offset: 20 }
		];
		expect(canRephrase(violations, [])).toBe(false);
	});

	it('cannot rephrase 3+ grounding issues', () => {
		const issues = [
			{ type: 'dose_mismatch' as const, claimed: 'a', cached: 'b', description: '1' },
			{ type: 'dose_mismatch' as const, claimed: 'c', cached: 'd', description: '2' },
			{ type: 'unknown_medication' as const, claimed: 'e', cached: null, description: '3' }
		];
		expect(canRephrase([], issues)).toBe(false);
	});
});

describe('safety-filter — rephrase engine', () => {
	it('removes prescriptive sentence', () => {
		const violations = scanRegexPatterns(
			'Your records show Metformin. You should stop taking it before surgery. Talk to your doctor.'
		);
		const result = rephraseResponse(
			'Your records show Metformin. You should stop taking it before surgery. Talk to your doctor.',
			violations,
			[]
		);
		expect(result).not.toContain('you should stop');
		expect(result).toContain('Metformin');
	});

	it('replaces mismatched dose with tab redirect', () => {
		const issues = [{
			type: 'dose_mismatch' as const,
			claimed: '500mg',
			cached: '250mg',
			description: 'Dose mismatch'
		}];
		const result = rephraseResponse(
			'Your Metformin dose is 500mg twice daily.',
			[],
			issues
		);
		expect(result).toContain('check Medications tab');
		expect(result).not.toContain('500mg');
	});

	it('falls back to blocked message when too little text remains', () => {
		const violations = [
			{ category: 'prescriptive' as const, matchedText: 'you should take aspirin', pattern: 'test', offset: 0 }
		];
		const result = rephraseResponse(
			'You should take aspirin.',
			violations,
			[]
		);
		expect(result).toBe(BLOCKED_FALLBACK_MESSAGE);
	});
});

// === FULL FILTER ===

describe('safety-filter — full filter', () => {
	it('passes clean response', () => {
		const result = filterResponse(
			'Based on your saved data, you are currently taking 3 active medications. Consider discussing this with your healthcare team.'
		);
		expect(result.outcome).toBe('passed');
	});

	it('rephrases minor prescriptive violation', () => {
		medications.set([makeMed()]);
		const result = filterResponse(
			'Your records show Metformin 500mg daily. You should take it with food. Your prescriber is Dr. Ndiaye.'
		);
		expect(result.outcome).toBe('rephrased');
		expect(result.text).not.toContain('you should take');
		expect(result.violations!.length).toBeGreaterThan(0);
	});

	it('blocks alarm language', () => {
		const result = filterResponse(
			'This is dangerous. You need to call 911 immediately.'
		);
		expect(result.outcome).toBe('blocked');
		expect(result.text).toBe(BLOCKED_FALLBACK_MESSAGE);
	});

	it('rephrases grounding dose mismatch', () => {
		medications.set([makeMed({ name: 'Lisinopril', dose: '10mg' })]);
		const result = filterResponse(
			'Your records show Lisinopril 20mg daily, prescribed by Dr. Ndiaye. Consider discussing this with your healthcare team.'
		);
		expect(result.outcome).toBe('rephrased');
		expect(result.text).toContain('check Medications tab');
		expect(result.groundingIssues!.length).toBeGreaterThan(0);
	});
});

// === EXTRACTION HELPERS ===

describe('safety-filter — extraction helpers', () => {
	it('extracts medication mentions with dose', () => {
		const mentions = extractMedicationMentions('Metformin 500mg and Lisinopril 10mg');
		expect(mentions).toHaveLength(2);
		expect(mentions[0].name).toBe('Metformin');
		expect(mentions[0].dose).toBe('500mg');
		expect(mentions[1].name).toBe('Lisinopril');
	});

	it('extracts lab mentions', () => {
		const mentions = extractLabMentions('HbA1c: 7.2 and Glucose 120');
		expect(mentions.length).toBeGreaterThanOrEqual(1);
		expect(mentions[0].testName.toLowerCase()).toBe('hba1c');
		expect(mentions[0].value).toBe('7.2');
	});

	it('normalizes dose strings', () => {
		expect(normalizeDose('500mg')).toBe('500 mg');
		expect(normalizeDose('10 mg')).toBe('10 mg');
		expect(normalizeDose('20mcg')).toBe('20 mcg');
		expect(normalizeDose('no dose here')).toBeNull();
	});
});
