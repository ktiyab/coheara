/**
 * L6-02: AI model management reactive store.
 *
 * Singleton store managing installed models, active model,
 * pull progress, and Ollama health state.
 */

import type { ModelInfo, ResolvedModel, ModelPullProgress, OllamaHealth } from '$lib/types/ai';
import { isMedicalModel } from '$lib/types/ai';

class AiStore {
	models = $state<ModelInfo[]>([]);
	activeModel = $state<ResolvedModel | null>(null);
	health = $state<OllamaHealth | null>(null);
	pullProgress = $state<ModelPullProgress | null>(null);
	loading = $state(false);
	error = $state<string | null>(null);

	get isPulling(): boolean {
		if (!this.pullProgress) return false;
		return (
			this.pullProgress.status !== 'complete' &&
			this.pullProgress.status !== 'error' &&
			this.pullProgress.status !== 'cancelled'
		);
	}

	get isOllamaReachable(): boolean {
		return this.health?.reachable ?? false;
	}

	get hasModels(): boolean {
		return this.models.length > 0;
	}

	get activeModelInfo(): ModelInfo | undefined {
		if (!this.activeModel) return undefined;
		return this.models.find((m) => m.name === this.activeModel!.name);
	}

	get isMedicalModelActive(): boolean {
		if (!this.activeModel) return false;
		return this.activeModel.quality === 'Medical';
	}

	/** Check if a model name is classified as medical. */
	isModelMedical(name: string): boolean {
		return isMedicalModel(name);
	}
}

export const ai = new AiStore();
