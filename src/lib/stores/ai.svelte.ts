/**
 * L6-02 + S.2/S.4/S.5/S.6: AI model management reactive store.
 *
 * Singleton store — single source of truth for all AI state:
 * models, active model, health, pull progress, status level, embedder type.
 *
 * S.5: Unifies profile.aiStatus and ai store — profile now derives from this.
 * S.2: Provides startPolling/stopPolling for periodic health checks.
 * S.4: Tracks real embedder type from backend.
 * S.6: Tracks status error state.
 */

import type { ModelInfo, ResolvedModel, ModelPullProgress, OllamaHealth } from '$lib/types/ai';
import { isMedicalModel } from '$lib/types/ai';
import type { StatusLevel, AiStatus } from '$lib/api/profile';

class AiStore {
	models = $state<ModelInfo[]>([]);
	activeModel = $state<ResolvedModel | null>(null);
	health = $state<OllamaHealth | null>(null);
	pullProgress = $state<ModelPullProgress | null>(null);
	loading = $state(false);
	error = $state<string | null>(null);

	// S.5: Unified status fields (previously duplicated in profile store)
	statusLevel = $state<StatusLevel>('unknown');
	embedderType = $state<string>('unknown');
	statusSummary = $state<string>('');
	// S.6: Status check error
	statusError = $state<string | null>(null);

	// S.2: Polling state
	private _pollTimer: ReturnType<typeof setInterval> | null = null;
	private _verifyTimer: ReturnType<typeof setTimeout> | null = null;

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

	/** S.5: Whether AI is available (unified check). */
	get isAiAvailable(): boolean {
		return this.isOllamaReachable && this.activeModel !== null;
	}

	/** Check if a model name is classified as medical. */
	isModelMedical(name: string): boolean {
		return isMedicalModel(name);
	}

	/** S.5: Apply backend AiStatus to unified store. */
	applyStatus(status: AiStatus): void {
		this.activeModel = status.active_model;
		this.embedderType = status.embedder_type;
		this.statusSummary = status.summary;
		this.statusLevel = status.level;
		this.statusError = null;

		// Sync health reachable from status
		if (this.health) {
			this.health = { ...this.health, reachable: status.ollama_available };
		} else {
			this.health = {
				reachable: status.ollama_available,
				version: null,
				models_count: 0,
			};
		}
	}

	/** S.6: Record a status check error. */
	setStatusError(error: string): void {
		this.statusError = error;
		this.statusLevel = 'error';
	}

	/** S.2: Start periodic polling. Calls checkFn immediately, verifyFn at 30s, then checkFn every 60s. */
	startPolling(
		checkFn: () => Promise<AiStatus>,
		verifyFn: () => Promise<AiStatus>,
	): void {
		this.stopPolling();

		// Immediate check
		checkFn()
			.then((status) => this.applyStatus(status))
			.catch((e) => this.setStatusError(String(e)));

		// S.2: Verify generation 30s after startup
		this._verifyTimer = setTimeout(() => {
			verifyFn()
				.then((status) => this.applyStatus(status))
				.catch((e) => this.setStatusError(String(e)));
		}, 30_000);

		// S.2: Health check every 60s
		this._pollTimer = setInterval(() => {
			checkFn()
				.then((status) => this.applyStatus(status))
				.catch((e) => this.setStatusError(String(e)));
		}, 60_000);
	}

	/** S.2: Stop polling. */
	stopPolling(): void {
		if (this._pollTimer) {
			clearInterval(this._pollTimer);
			this._pollTimer = null;
		}
		if (this._verifyTimer) {
			clearTimeout(this._verifyTimer);
			this._verifyTimer = null;
		}
	}
}

export const ai = new AiStore();
