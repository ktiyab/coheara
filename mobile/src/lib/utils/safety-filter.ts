// M2-03: Safety Filter — 24 regex patterns + factual grounding check + rephrase/block
import { get } from 'svelte/store';
import type {
	SafetyPattern,
	PhoneViolation,
	GroundingIssue,
	PhoneFilterResult,
	MedicationMention,
	LabMention
} from '$lib/types/safety-filter.js';
import { BLOCKED_FALLBACK_MESSAGE } from '$lib/types/safety-filter.js';
import { medications, labResults } from '$lib/stores/cache.js';

// ─────────────────────────────────────────────────────────────
// DIAGNOSTIC PATTERNS (8) — mirrors L2-02 DIAGNOSTIC_PATTERNS
// ─────────────────────────────────────────────────────────────

const DIAGNOSTIC_PATTERNS: SafetyPattern[] = [
	{
		regex: /\byou\s+have\s+(?:a\s+)?(?:been\s+)?(?:diagnosed\s+with\s+)?[a-z]/i,
		category: 'diagnostic',
		description: "Direct diagnosis: 'you have [condition]'"
	},
	{
		regex: /\byou\s+are\s+suffering\s+from\b/i,
		category: 'diagnostic',
		description: "Direct diagnosis: 'you are suffering from'"
	},
	{
		regex: /\byou\s+(?:likely|probably|possibly)\s+have\b/i,
		category: 'diagnostic',
		description: "Speculative diagnosis: 'you likely/probably have'"
	},
	{
		regex: /\bthis\s+(?:means|indicates|suggests|confirms)\s+(?:you|that\s+you)\s+have\b/i,
		category: 'diagnostic',
		description: "Indirect diagnosis: 'this means you have'"
	},
	{
		regex: /\byou\s+(?:are|have\s+been)\s+diagnosed\b/i,
		category: 'diagnostic',
		description: "Diagnosis claim without document attribution"
	},
	{
		regex: /\byou(?:'re|\s+are)\s+(?:a\s+)?diabetic\b/i,
		category: 'diagnostic',
		description: "Direct label: 'you are diabetic'"
	},
	{
		regex: /\byour\s+condition\s+is\b/i,
		category: 'diagnostic',
		description: "Condition assertion: 'your condition is'"
	},
	{
		regex: /\byou\s+(?:appear|seem)\s+to\s+have\b/i,
		category: 'diagnostic',
		description: "Implied diagnosis: 'you appear to have'"
	}
];

// ─────────────────────────────────────────────────────────────
// PRESCRIPTIVE PATTERNS (8) — mirrors L2-02 PRESCRIPTIVE_PATTERNS
// ─────────────────────────────────────────────────────────────

const PRESCRIPTIVE_PATTERNS: SafetyPattern[] = [
	{
		regex: /\byou\s+should\s+(?:take|stop|start|increase|decrease|change|switch|discontinue|avoid|reduce)\b/i,
		category: 'prescriptive',
		description: "Direct prescription: 'you should [take/stop/...]'"
	},
	{
		regex: /\bI\s+recommend\b/i,
		category: 'prescriptive',
		description: "Direct recommendation: 'I recommend'"
	},
	{
		regex: /\bI\s+(?:would\s+)?(?:suggest|advise)\b/i,
		category: 'prescriptive',
		description: "Advisory language: 'I suggest/advise'"
	},
	{
		regex: /\byou\s+(?:need|must|have)\s+to\s+(?:take|stop|start|see|visit|go|call|increase|decrease)\b/i,
		category: 'prescriptive',
		description: "Imperative prescription: 'you need to [action]'"
	},
	{
		regex: /\bdo\s+not\s+(?:take|stop|eat|drink|use|skip)\b/i,
		category: 'prescriptive',
		description: "Prohibition: 'do not [action]'"
	},
	{
		regex: /\btry\s+(?:taking|using|adding|reducing)\b/i,
		category: 'prescriptive',
		description: "Soft prescription: 'try taking/using'"
	},
	{
		regex: /\bthe\s+(?:best|recommended)\s+(?:treatment|course\s+of\s+action|approach)\s+(?:is|would\s+be)\b/i,
		category: 'prescriptive',
		description: "Treatment recommendation: 'the best treatment is'"
	},
	{
		regex: /\bconsider\s+(?:taking|stopping|increasing|decreasing|switching)\b/i,
		category: 'prescriptive',
		description: "Soft prescription: 'consider taking/stopping'"
	}
];

// ─────────────────────────────────────────────────────────────
// ALARM PATTERNS (8) — mirrors L2-02 ALARM_PATTERNS (NC-07)
// ─────────────────────────────────────────────────────────────

const ALARM_PATTERNS: SafetyPattern[] = [
	{
		regex: /\b(?:dangerous|life[- ]threatening|fatal|deadly|lethal)\b/i,
		category: 'alarm',
		description: "Alarm word: dangerous/life-threatening/fatal"
	},
	{
		regex: /\b(?:emergency|urgent(?:ly)?|immediately|right\s+away|right\s+now)\b/i,
		category: 'alarm',
		description: "Urgency word: emergency/immediately/urgently"
	},
	{
		regex: /\b(?:immediately|urgently)\s+(?:go|call|visit|see|seek|get)\b/i,
		category: 'alarm',
		description: "Urgent directive: 'immediately go/call'"
	},
	{
		regex: /\bcall\s+(?:911|emergency|an\s+ambulance|your\s+doctor\s+(?:immediately|right\s+away|now))\b/i,
		category: 'alarm',
		description: "Emergency call directive: 'call 911/emergency'"
	},
	{
		regex: /\bgo\s+to\s+(?:the\s+)?(?:emergency|ER|hospital|A&E)\b/i,
		category: 'alarm',
		description: "ER directive: 'go to the emergency/hospital'"
	},
	{
		regex: /\bseek\s+(?:immediate|emergency|urgent)\s+(?:medical\s+)?(?:help|attention|care)\b/i,
		category: 'alarm',
		description: "Seek care directive: 'seek immediate medical help'"
	},
	{
		regex: /\bthis\s+(?:is|could\s+be)\s+(?:a\s+)?(?:medical\s+)?emergency\b/i,
		category: 'alarm',
		description: "Emergency declaration: 'this is an emergency'"
	},
	{
		regex: /\bdo\s+not\s+(?:wait|delay|ignore)\b/i,
		category: 'alarm',
		description: "Urgency pressure: 'do not wait/delay'"
	}
];

/** All 24 patterns combined */
export const ALL_PATTERNS: SafetyPattern[] = [
	...DIAGNOSTIC_PATTERNS,
	...PRESCRIPTIVE_PATTERNS,
	...ALARM_PATTERNS
];

// === REGEX SCAN ===

/** Scan response text for safety violations */
export function scanRegexPatterns(text: string): PhoneViolation[] {
	const violations: PhoneViolation[] = [];

	for (const pattern of ALL_PATTERNS) {
		const regex = new RegExp(pattern.regex.source, pattern.regex.flags.replace('g', '') + 'g');
		let match: RegExpExecArray | null;

		while ((match = regex.exec(text)) !== null) {
			violations.push({
				category: pattern.category,
				matchedText: match[0],
				pattern: pattern.description,
				offset: match.index
			});
		}
	}

	return deduplicateViolations(violations);
}

/** Remove overlapping violations, keeping the longer match */
function deduplicateViolations(violations: PhoneViolation[]): PhoneViolation[] {
	violations.sort((a, b) => a.offset - b.offset || b.matchedText.length - a.matchedText.length);

	const result: PhoneViolation[] = [];
	for (const v of violations) {
		const last = result[result.length - 1];
		if (last && v.offset < last.offset + last.matchedText.length) {
			continue;
		}
		result.push(v);
	}
	return result;
}

// === GROUNDING CHECK ===

/** Non-medication words to filter out of extraction */
const NON_MEDICATION_WORDS = new Set([
	'based', 'your', 'the', 'this', 'that', 'from', 'with',
	'about', 'records', 'data', 'saved', 'medications', 'results',
	'currently', 'taking', 'prescribed', 'daily', 'twice', 'once',
	'doctor', 'visit', 'appointment', 'consider', 'discussing',
	'shows', 'show', 'include', 'includes', 'according', 'tab',
	'check', 'for', 'exact', 'dose', 'information', 'health'
]);

/** Extract medication name + dose mentions from text */
export function extractMedicationMentions(text: string): MedicationMention[] {
	const mentions: MedicationMention[] = [];
	const pattern = /\b([A-Z][a-z]{2,}(?:\s+[A-Z][a-z]+)*)\s+(\d+(?:\.\d+)?\s*(?:mg|mcg|units?|ml|g)\b)/g;
	let match: RegExpExecArray | null;

	while ((match = pattern.exec(text)) !== null) {
		const name = match[1].trim();
		const dose = match[2]?.trim() || null;

		if (!NON_MEDICATION_WORDS.has(name.toLowerCase())) {
			mentions.push({ name, dose });
		}
	}

	return mentions;
}

/** Extract lab test name + value mentions from text */
export function extractLabMentions(text: string): LabMention[] {
	const mentions: LabMention[] = [];
	const knownTests = 'HbA1c|Potassium|Sodium|Glucose|Cholesterol|Hemoglobin|Creatinine|TSH|T4|HDL|LDL|Triglycerides';

	const patterns = [
		new RegExp(`\\b(${knownTests})[\\s:]+(\\d+(?:\\.\\d+)?)`, 'gi'),
		new RegExp(`\\b(${knownTests})\\s+(?:of|is|was|at)\\s+(\\d+(?:\\.\\d+)?)`, 'gi')
	];

	const seen = new Set<string>();
	for (const p of patterns) {
		let match: RegExpExecArray | null;
		while ((match = p.exec(text)) !== null) {
			const key = `${match[1].toLowerCase()}-${match[2]}`;
			if (!seen.has(key)) {
				seen.add(key);
				mentions.push({ testName: match[1], value: match[2] });
			}
		}
	}

	return mentions;
}

/** Normalize dose strings for comparison */
export function normalizeDose(dose: string): string | null {
	const match = dose.match(/(\d+(?:\.\d+)?)\s*(mg|mcg|units?|ml|g)\b/i);
	if (!match) return null;
	return `${match[1]} ${match[2].toLowerCase()}`;
}

/** Cross-reference SLM response against cached medications */
export function checkMedicationGrounding(response: string): GroundingIssue[] {
	const issues: GroundingIssue[] = [];
	const cachedMeds = get(medications);

	if (cachedMeds.length === 0) return issues;

	const medMentions = extractMedicationMentions(response);

	for (const mention of medMentions) {
		const cached = cachedMeds.find(
			m => m.name.toLowerCase() === mention.name.toLowerCase()
		);

		if (!cached) {
			issues.push({
				type: 'unknown_medication',
				claimed: mention.name,
				cached: null,
				description: `SLM mentioned "${mention.name}" which is not in the cached medication list`
			});
			continue;
		}

		if (mention.dose) {
			const claimedDose = normalizeDose(mention.dose);
			const cachedDose = normalizeDose(cached.dose);

			if (claimedDose && cachedDose && claimedDose !== cachedDose) {
				issues.push({
					type: 'dose_mismatch',
					claimed: mention.dose,
					cached: cached.dose,
					description: `SLM stated dose "${mention.dose}" but cache shows "${cached.dose}"`
				});
			}
		}
	}

	return issues;
}

/** Cross-reference SLM response against cached lab results */
export function checkLabGrounding(response: string): GroundingIssue[] {
	const issues: GroundingIssue[] = [];
	const cachedLabs = get(labResults);

	if (cachedLabs.length === 0) return issues;

	const labMentions = extractLabMentions(response);

	for (const mention of labMentions) {
		const cached = cachedLabs.find(
			l => l.testName.toLowerCase() === mention.testName.toLowerCase()
		);

		if (mention.value && cached) {
			const claimedValue = parseFloat(mention.value);
			const cachedValue = cached.value;

			if (!isNaN(claimedValue) && claimedValue !== cachedValue) {
				issues.push({
					type: 'value_mismatch',
					claimed: `${mention.testName}: ${mention.value}`,
					cached: `${cached.testName}: ${cached.value} ${cached.unit}`,
					description: `SLM stated ${mention.testName} value "${mention.value}" but cache shows "${cached.value}"`
				});
			}
		}
	}

	return issues;
}

/** Run full grounding check */
export function checkGrounding(response: string): GroundingIssue[] {
	return [
		...checkMedicationGrounding(response),
		...checkLabGrounding(response)
	];
}

// === REPHRASE / BLOCK ===

/** Determine if violations are minor enough to rephrase */
export function canRephrase(violations: PhoneViolation[], groundingIssues: GroundingIssue[]): boolean {
	if (violations.some(v => v.category === 'alarm')) return false;
	if (violations.length > 2) return false;
	if (groundingIssues.length > 2) return false;
	return true;
}

function escapeRegex(str: string): string {
	return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/** Attempt to salvage a response with minor violations */
export function rephraseResponse(
	original: string,
	violations: PhoneViolation[],
	groundingIssues: GroundingIssue[]
): string {
	let text = original;

	// Handle grounding issues: remove specific values
	for (const issue of groundingIssues) {
		if (issue.type === 'dose_mismatch') {
			text = text.replace(
				new RegExp(escapeRegex(issue.claimed), 'gi'),
				'[check Medications tab for exact dose]'
			);
		}
		if (issue.type === 'unknown_medication') {
			text = text.replace(
				new RegExp(`\\b${escapeRegex(issue.claimed)}\\b`, 'gi'),
				'[medication not in your saved data]'
			);
		}
	}

	// Handle regex violations: remove violating sentences
	for (const violation of violations) {
		if (violation.category === 'prescriptive' || violation.category === 'diagnostic') {
			const sentencePattern = new RegExp(
				`[^.!?]*${escapeRegex(violation.matchedText)}[^.!?]*[.!?]?`,
				'gi'
			);
			text = text.replace(sentencePattern, '');
		}
	}

	// Clean up whitespace
	text = text.replace(/\n{3,}/g, '\n\n').trim();

	// If rephrase left too little content, fall back
	if (text.length < 20) {
		return BLOCKED_FALLBACK_MESSAGE;
	}

	// Add safe closing if needed
	if (!text.includes('healthcare team') && !text.includes('desktop')) {
		text += '\n\nConsider discussing your health data with your healthcare team.';
	}

	return text;
}

// === MAIN FILTER ===

/** Run all safety checks on an SLM response */
export function filterResponse(response: string): PhoneFilterResult {
	// Layer 2: Regex scan
	const regexViolations = scanRegexPatterns(response);

	// Grounding check: cross-reference cached data
	const groundingIssues = checkGrounding(response);

	// No issues → pass
	if (regexViolations.length === 0 && groundingIssues.length === 0) {
		return { outcome: 'passed', text: response };
	}

	// Attempt rephrase for minor violations
	if (canRephrase(regexViolations, groundingIssues)) {
		const rephrased = rephraseResponse(response, regexViolations, groundingIssues);
		return {
			outcome: 'rephrased',
			text: rephrased,
			violations: regexViolations,
			groundingIssues
		};
	}

	// Block for severe violations
	return {
		outcome: 'blocked',
		text: BLOCKED_FALLBACK_MESSAGE,
		violations: regexViolations,
		groundingIssues
	};
}
