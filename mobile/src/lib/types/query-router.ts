// M2-02: Query Router types — routing decisions, safety blocklist, cache matching
import type { CacheScope } from './slm.js';

// === ROUTE TYPES ===

export type QueryRoute =
	| { target: 'desktop' }
	| { target: 'slm'; cacheScope: CacheScope; confidence: 'high' | 'low' }
	| { target: 'deferred'; query: string }
	| { target: 'fallback_tab'; tab: string }
	| { target: 'safety_blocked'; reason: string; message: string };

// === SAFETY BLOCKLIST ===

export type SafetyCategory =
	| 'drug_interactions'
	| 'dosage_change'
	| 'symptom_assessment'
	| 'side_effects'
	| 'treatment_advice'
	| 'emergency';

export interface SafetyCheckResult {
	blocked: boolean;
	desktopAllowed?: boolean;
	category?: SafetyCategory;
	reason?: string;
	userMessage?: string;
}

// === CACHE MATCHING ===

export interface CacheMatchResult {
	scope: CacheScope;
	confidence: number; // 0.0 - 1.0
}

// === QUICK-QUESTION CHIPS ===

export interface QuickQuestion {
	label: string;
	query: string;
	preClassified: CacheMatchResult | null;
	slmCapable: boolean;
}

// === SAFETY MESSAGES ===

export const SAFETY_MESSAGES: Record<SafetyCategory, string> = {
	drug_interactions: "Drug interaction analysis requires your desktop's full records. Connect to your desktop for this answer.",
	dosage_change: "Dosage decisions should be made with your healthcare team. Coheara can show what's currently prescribed.",
	symptom_assessment: 'For symptom assessment, please consult your healthcare team. Coheara can help you prepare for that conversation.',
	side_effects: "Side effect analysis requires your desktop's complete records. Connect to your desktop.",
	treatment_advice: 'Treatment decisions should be discussed with your healthcare team.',
	emergency: "If you're experiencing a medical emergency, please call emergency services."
};

// === SAFETY REASONS ===

export const SAFETY_REASONS: Record<SafetyCategory, string> = {
	drug_interactions: 'Requires full medication database and interaction checking',
	dosage_change: 'Clinical decision requiring healthcare professional',
	symptom_assessment: 'Clinical assessment beyond AI scope',
	side_effects: 'Requires full medication knowledge',
	treatment_advice: 'NC-02: No clinical advice',
	emergency: 'Potential medical emergency'
};

/** Categories where desktop RAG CAN answer (safety topic but data-driven) */
export const DESKTOP_ALLOWED_CATEGORIES: SafetyCategory[] = [
	'drug_interactions',
	'side_effects'
];

// === KEYWORD SETS ===

export const MEDICATION_KEYWORDS = [
	'medication', 'medicine', 'med', 'pill', 'drug', 'prescription',
	'taking', 'prescribed', 'dose', 'dosage', 'pharmacy',
	'metformin', 'lisinopril', 'insulin'
];

export const LAB_KEYWORDS = [
	'lab', 'test', 'result', 'blood', 'urine', 'value',
	'hba1c', 'glucose', 'cholesterol', 'potassium', 'sodium',
	'hemoglobin', 'creatinine', 'thyroid'
];

export const TIMELINE_KEYWORDS = [
	'when', 'last', 'history', 'timeline', 'date', 'started',
	'changed', 'previous', 'recent', 'ago'
];

export const APPOINTMENT_KEYWORDS = [
	'appointment', 'doctor', 'visit', 'checkup', 'scheduled',
	'next', 'upcoming', 'specialist'
];

export const ALERT_KEYWORDS = [
	'alert', 'warning', 'notice', 'flag', 'attention',
	'abnormal', 'concern'
];

// === SAFETY PATTERNS ===

export const INTERACTION_PATTERNS: RegExp[] = [
	/\b(?:drug|medication|med)\s+interaction/i,
	/\binteract(?:s|ion)?\s+with\b/i,
	/\btaken?\s+(?:with|together|alongside)\b/i,
	/\bcombine|combining|combination\b/i,
	/\bcontraindicated?\b/i,
	/\bmix(?:ing)?\s+(?:with|medications?|drugs?|meds?)\b/i,
];

export const DOSAGE_CHANGE_PATTERNS: RegExp[] = [
	/\b(?:change|increase|decrease|reduce|adjust|modify|double|half)\s+(?:my\s+)?(?:dose|dosage|medication|med)\b/i,
	/\bshould\s+i\s+(?:take|stop|start|change|increase|decrease|skip)\b/i,
	/\b(?:stop|quit|discontinue)\s+(?:taking|my|the)\b/i,
	/\btoo\s+(?:much|little|high|low)\s+(?:dose|dosage|medication)?\b/i,
	/\bmissed?\s+(?:a\s+)?dose\b/i,
];

export const SYMPTOM_PATTERNS: RegExp[] = [
	/\bwhat\s+(?:does|could|might)\s+(?:this|my)\s+(?:symptom|pain|feeling)\b/i,
	/\bis\s+(?:this|it)\s+(?:normal|serious|dangerous|concerning)\b/i,
	/\bshould\s+i\s+(?:be\s+)?(?:worried|concerned)\b/i,
	/\bwhat(?:'s|\s+is)\s+wrong\s+with\s+me\b/i,
	/\bdiagnos(?:e|is)\b/i,
];

export const SIDE_EFFECT_PATTERNS: RegExp[] = [
	/\bside\s+effect/i,
	/\badverse\s+(?:effect|reaction|event)\b/i,
	/\b(?:caused?|causing)\s+by\s+(?:my\s+)?(?:medication|med|drug|pill)\b/i,
	/\breaction\s+to\s+(?:my\s+)?(?:medication|med|drug|pill)\b/i,
];

export const TREATMENT_PATTERNS: RegExp[] = [
	/\bwhat\s+(?:treatment|therapy|remedy|cure)\b/i,
	/\bhow\s+(?:to\s+)?(?:treat|cure|fix|heal)\b/i,
	/\bwhat\s+should\s+(?:i|we)\s+do\s+about\b/i,
	/\balternative\s+(?:treatment|medication|therapy)\b/i,
];

export const EMERGENCY_PATTERNS: RegExp[] = [
	/\b(?:emergency|911|ambulance|er\b|a&e)\b/i,
	/\bcan(?:'t|not)\s+breathe?\b/i,
	/\bchest\s+pain\b/i,
	/\bsuicid(?:e|al)\b/i,
	/\boverdos(?:e|ed|ing)\b/i,
	/\bsevere\s+(?:pain|bleeding|reaction)\b/i,
];

// === QUICK-QUESTION CHIP DEFINITIONS ===

export const QUICK_QUESTION_CHIPS: QuickQuestion[] = [
	{
		label: 'My medications',
		query: 'What are my current medications?',
		preClassified: {
			scope: { medications: true, labs: false, timeline: false, alerts: false, appointment: false, profile: true },
			confidence: 1.0
		},
		slmCapable: true
	},
	{
		label: 'Next appointment',
		query: 'When is my next doctor appointment?',
		preClassified: {
			scope: { medications: false, labs: false, timeline: false, alerts: false, appointment: true, profile: true },
			confidence: 1.0
		},
		slmCapable: true
	},
	{
		label: 'Recent lab results',
		query: 'What are my most recent lab results?',
		preClassified: {
			scope: { medications: false, labs: true, timeline: false, alerts: false, appointment: false, profile: true },
			confidence: 1.0
		},
		slmCapable: true
	},
	{
		label: 'What to ask my doctor',
		query: 'What questions should I prepare for my next doctor appointment?',
		preClassified: null,
		slmCapable: false
	},
	{
		label: 'Active alerts',
		query: 'Are there any active health alerts?',
		preClassified: {
			scope: { medications: false, labs: false, timeline: false, alerts: true, appointment: false, profile: true },
			confidence: 1.0
		},
		slmCapable: true
	}
];
