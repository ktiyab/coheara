// M2-01: SLM types — model lifecycle, generation, context assembly

// === MODEL IDENTITY ===

export type ModelChoice = 'gemma-2b-q4' | 'gemma-2b-q5' | 'phi3-mini-q4';

export interface ModelSpec {
	id: ModelChoice;
	name: string;
	sizeBytes: number;
	ramRequired: number;
	quantization: string;
	tokensPerSecond: number;
}

export const MODEL_SPECS: Record<ModelChoice, ModelSpec> = {
	'gemma-2b-q4': {
		id: 'gemma-2b-q4',
		name: 'Gemma 2B (Q4)',
		sizeBytes: 1.5 * 1024 * 1024 * 1024,
		ramRequired: 2.0 * 1024 * 1024 * 1024,
		quantization: 'Q4_K_M',
		tokensPerSecond: 8
	},
	'gemma-2b-q5': {
		id: 'gemma-2b-q5',
		name: 'Gemma 2B (Q5)',
		sizeBytes: 1.8 * 1024 * 1024 * 1024,
		ramRequired: 2.3 * 1024 * 1024 * 1024,
		quantization: 'Q5_K_M',
		tokensPerSecond: 6
	},
	'phi3-mini-q4': {
		id: 'phi3-mini-q4',
		name: 'Phi-3 Mini (Q4)',
		sizeBytes: 2.0 * 1024 * 1024 * 1024,
		ramRequired: 2.5 * 1024 * 1024 * 1024,
		quantization: 'Q4_K_M',
		tokensPerSecond: 6
	}
};

export const DEFAULT_MODEL: ModelChoice = 'gemma-2b-q4';

// === MODEL STATE MACHINE ===

export type ModelState =
	| 'not_capable'
	| 'not_downloaded'
	| 'downloading'
	| 'downloaded'
	| 'loading'
	| 'ready'
	| 'generating';

export interface ModelInfo {
	id: ModelChoice;
	name: string;
	sizeBytes: number;
	quantization: string;
	loaded: boolean;
	lastUsed: string | null;
}

// === GENERATION ===

export interface GenerateOptions {
	maxTokens: number;
	temperature: number;
	topP: number;
	stopSequences: string[];
}

export const DEFAULT_GENERATE_OPTIONS: GenerateOptions = {
	maxTokens: 512,
	temperature: 0.3,
	topP: 0.9,
	stopSequences: ['\n\nUser:', '\n\nHuman:']
};

export type FinishReason = 'stop' | 'max_tokens' | 'aborted';

export interface SlmResult {
	tokensGenerated: number;
	timeMs: number;
	tokensPerSecond: number;
	finishReason: FinishReason;
}

// === DOWNLOAD ===

export interface DownloadProgress {
	percent: number;
	downloadedBytes: number;
	totalBytes: number;
}

// === CONTEXT ASSEMBLY ===

export interface CacheScope {
	medications: boolean;
	labs: boolean;
	timeline: boolean;
	alerts: boolean;
	appointment: boolean;
	profile: boolean;
}

/** Include all cache sections in SLM context */
export function fullCacheScope(): CacheScope {
	return { medications: true, labs: true, timeline: true, alerts: true, appointment: true, profile: true };
}

/** Include only medications in SLM context */
export function medicationScope(): CacheScope {
	return { medications: true, labs: false, timeline: false, alerts: false, appointment: false, profile: true };
}

/** Include only labs in SLM context */
export function labScope(): CacheScope {
	return { medications: false, labs: true, timeline: false, alerts: false, appointment: false, profile: true };
}

// === DEVICE CAPABILITY ===

export const MIN_RAM_BYTES = 6 * 1024 * 1024 * 1024;  // 6 GB
export const MIN_STORAGE_BYTES = 3 * 1024 * 1024 * 1024;  // 3 GB free
export const MIN_CHAT_SESSIONS_FOR_PROMOTION = 3;
export const BACKGROUND_UNLOAD_MS = 30 * 1000;  // 30 seconds
export const CONTEXT_TOKEN_LIMIT = 1500;
export const RESPONSE_TOKEN_LIMIT = 512;

// === SLM SYSTEM PROMPT ===

export const SLM_SYSTEM_PROMPT = `You are Coheara, a patient health data assistant. You help patients understand their medical records by answering questions about their health data.

RULES — Follow these exactly:
1. ONLY use the data provided below. Do not invent, guess, or extrapolate.
2. If the data does not contain the answer, say: "I don't have that information in your saved data. Try connecting to your desktop for a complete answer."
3. Always frame answers as: "Based on your saved data..." or "Your records show..."
4. NEVER say "you have [condition]" — say "your records mention [condition]"
5. NEVER give medical advice, recommend treatments, or suggest dosage changes.
6. NEVER use alarm language: no "dangerous", "emergency", "immediately", "urgent".
7. For any question about drug interactions, side effects, or symptoms: say "This requires your desktop's full analysis. Please connect to your desktop or consult your healthcare team."
8. End clinical mentions with: "Consider discussing this with your healthcare team."
9. Keep responses concise — under 150 words.
10. Include the data freshness in your response when relevant.`;
