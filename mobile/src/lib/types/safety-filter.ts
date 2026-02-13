// M2-03: Safety Filter types — violation categories, grounding issues, filter results

// === VIOLATION TYPES ===

export type ViolationCategory = 'diagnostic' | 'prescriptive' | 'alarm';

export interface SafetyPattern {
	regex: RegExp;
	category: ViolationCategory;
	description: string;
}

export interface PhoneViolation {
	category: ViolationCategory;
	matchedText: string;
	pattern: string;
	offset: number;
}

// === GROUNDING TYPES ===

export type GroundingIssueType = 'dose_mismatch' | 'unknown_medication' | 'value_mismatch';

export interface GroundingIssue {
	type: GroundingIssueType;
	claimed: string;
	cached: string | null;
	description: string;
}

// === FILTER RESULT ===

export type FilterOutcome = 'passed' | 'rephrased' | 'blocked';

export interface PhoneFilterResult {
	outcome: FilterOutcome;
	text: string;
	violations?: PhoneViolation[];
	groundingIssues?: GroundingIssue[];
}

// === AUDIT LOGGING ===

export interface SafetyAuditEntry {
	timestamp: string;
	queryHash: string;
	outcome: FilterOutcome;
	violationsCount: number;
	groundingIssuesCount: number;
	categories: string[];
}

// === CONSTANTS ===

export const BLOCKED_FALLBACK_MESSAGE =
	"I'd rather give you a thorough answer on this one. " +
	"Try asking when connected to your desktop.";

// === EXTRACTION HELPERS ===

export interface MedicationMention {
	name: string;
	dose: string | null;
}

export interface LabMention {
	testName: string;
	value: string;
}
