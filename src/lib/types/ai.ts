/**
 * L6-02: AI Model Management TypeScript types.
 *
 * Maps to Rust structs in:
 * - pipeline/structuring/ollama_types.rs (ModelInfo, ModelDetail, etc.)
 * - pipeline/structuring/preferences.rs (ResolvedModel, ModelQuality, etc.)
 * - commands/ai_setup.rs (ModelPullProgress event payload)
 */

// ── Model Info (from Ollama /api/tags) ──────────────────────

export interface ModelDetails {
	family: string | null;
	parameter_size: string | null;
	quantization_level: string | null;
}

export interface ModelInfo {
	name: string;
	size: number;
	digest: string;
	modified_at: string;
	details: ModelDetails;
}

// ── Model Detail (from Ollama /api/show) ────────────────────

export interface ModelDetail {
	name: string;
	modelfile: string;
	parameters: string;
	template: string;
	details: ModelDetails;
}

// ── Recommended Models (curated medical list) ───────────────

export interface RecommendedModel {
	name: string;
	description: string;
	min_ram_gb: number;
	medical: boolean;
}

// ── Ollama Health ───────────────────────────────────────────

export interface OllamaHealth {
	reachable: boolean;
	version: string | null;
	models_count: number;
}

// ── Model Preferences (L6-04) ──────────────────────────────

export type ModelQuality = 'Medical' | 'General' | 'Unknown';
export type PreferenceSource = 'User' | 'Wizard' | 'Fallback';

export interface ResolvedModel {
	name: string;
	quality: ModelQuality;
	source: PreferenceSource;
}

// ── Pull Progress (Tauri event payload) ─────────────────────

export interface ModelPullProgress {
	status: string;
	model_name: string;
	progress_percent: number;
	bytes_completed: number;
	bytes_total: number;
	error_message: string | null;
}

// ── Derived helpers ─────────────────────────────────────────

/** Medical model name prefixes — mirrors Rust MEDICAL_MODEL_PREFIXES. */
const MEDICAL_PREFIXES = [
	'medgemma',
	'biomistral',
	'meditron',
	'med-',
	'medical',
	'biomedical',
	'clinical',
	'pubmed'
];

/** Check if a model name is classified as medical. */
export function isMedicalModel(name: string): boolean {
	const lower = name.toLowerCase();
	return MEDICAL_PREFIXES.some((prefix) => lower.startsWith(prefix));
}

/** Format bytes to human-readable size (e.g., "2.5 GB"). */
export function formatModelSize(bytes: number): string {
	if (bytes >= 1_000_000_000) {
		return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
	}
	if (bytes >= 1_000_000) {
		return `${(bytes / 1_000_000).toFixed(0)} MB`;
	}
	return `${(bytes / 1_000).toFixed(0)} KB`;
}

/** Human-readable source text for preference display. */
export function sourceDisplayText(source: PreferenceSource): string {
	switch (source) {
		case 'User':
			return 'Set by you';
		case 'Wizard':
			return 'Set during AI setup';
		case 'Fallback':
			return 'Auto-selected';
	}
}
