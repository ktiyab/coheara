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

// ── Hardware Profile ────────────────────────────────────────

export type GpuTier = 'full_gpu' | 'partial_gpu' | 'cpu_only';

export interface HardwareStatus {
	gpu_tier: GpuTier;
	gpu_available: boolean;
	vram_bytes: number;
	total_model_bytes: number;
	processor_label: string;
	detected_at: string;
	estimated_tok_per_sec: number;
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

/**
 * Extract the model component from a full model name.
 * Strips namespace prefix and tag suffix: "namespace/model:tag" → "model"
 * Mirrors Rust extract_model_component() in ollama_types.rs.
 */
export function extractModelComponent(fullName: string): string {
	const withoutTag = fullName.split(':')[0] ?? fullName;
	const parts = withoutTag.split('/');
	return (parts[parts.length - 1] ?? withoutTag).toLowerCase();
}

/** Check if a model name is classified as medical. */
export function isMedicalModel(name: string): boolean {
	const component = extractModelComponent(name);
	return MEDICAL_PREFIXES.some((prefix) => component.startsWith(prefix));
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
