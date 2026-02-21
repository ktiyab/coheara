/**
 * L6-02 + S.2/S.4/S.5/S.6: AI model management reactive store.
 *
 * Singleton store — single source of truth for all AI state:
 * models, active model, health, pull progress, status level, embedder type.
 *
 * S.5: Unifies profile.aiStatus and ai store — profile now derives from this.
 * S.2: Provides startupCheck for one-shot initialization (replaced 60s polling).
 * S.4: Tracks real embedder type from backend.
 * S.6: Tracks status error state + on-failure updates from real operations.
 *
 * Architecture: event-driven, not polling.
 *   - Startup: checkFn (immediate) + verifyFn (30s delay)
 *   - Runtime: handleOperationFailure() called by chat/import on AI error
 *   - No recurring intervals — Ollama is local and user-controlled.
 */

import type { ModelInfo, ResolvedModel, ModelPullProgress, OllamaHealth } from '$lib/types/ai';
import { isMedicalModel } from '$lib/types/ai';
import type { StatusLevel, AiStatus } from '$lib/api/profile';
import { classifyAiFailure } from '$lib/utils/ai-failure';

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

	// Startup verify timer (one-shot, not recurring)
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

	/**
	 * One-shot startup initialization. Replaces the former 60s polling loop.
	 *
	 * - checkFn: immediate — establishes baseline (is Ollama running? which model?).
	 * - verifyFn: 30s delay — confirms the model can actually generate.
	 *
	 * After startup, status is only updated by handleOperationFailure() when
	 * a real AI operation (chat, document processing) encounters an error.
	 */
	startupCheck(
		checkFn: () => Promise<AiStatus>,
		verifyFn: () => Promise<AiStatus>,
	): void {
		this.cleanup();

		// Immediate baseline check
		checkFn()
			.then((status) => this.applyStatus(status))
			.catch((e) => this.setStatusError(String(e)));

		// Delayed verification (one-shot, not recurring)
		this._verifyTimer = setTimeout(() => {
			verifyFn()
				.then((status) => this.applyStatus(status))
				.catch((e) => this.setStatusError(String(e)));
			this._verifyTimer = null;
		}, 30_000);
	}

	/**
	 * Handle a failure from a real AI operation (chat send, document processing).
	 *
	 * Classifies the error and updates status reactively. This is the runtime
	 * monitor — no polling needed because real operations are the signal.
	 *
	 * @param error - The caught error from sendChatMessage, processDocument, etc.
	 */
	handleOperationFailure(error: unknown): void {
		const kind = classifyAiFailure(error);

		if (kind === 'unrelated') return;

		const msg = error instanceof Error ? error.message : String(error);

		switch (kind) {
			case 'ollama_unreachable':
				this.statusLevel = 'error';
				this.statusSummary = 'Ollama not detected — check that Ollama is running';
				if (this.health) {
					this.health = { ...this.health, reachable: false };
				} else {
					this.health = { reachable: false, version: null, models_count: 0 };
				}
				break;

			case 'model_not_found':
				this.statusLevel = 'error';
				this.activeModel = null;
				this.statusSummary = 'Model not available — check AI settings';
				break;

			case 'generation_failed':
				this.statusLevel = 'degraded';
				this.statusSummary = `AI generation failed — ${msg}`;
				break;

			case 'timeout':
				this.statusLevel = 'degraded';
				this.statusSummary = `AI request timed out — ${msg}`;
				break;
		}
	}

	/** Clean up timers. Call on component destroy. */
	cleanup(): void {
		if (this._verifyTimer) {
			clearTimeout(this._verifyTimer);
			this._verifyTimer = null;
		}
	}

	// ── Backwards compatibility aliases ──────────────────────

	/** @deprecated Use startupCheck instead. Kept for call-site migration. */
	startPolling(
		checkFn: () => Promise<AiStatus>,
		verifyFn: () => Promise<AiStatus>,
	): void {
		this.startupCheck(checkFn, verifyFn);
	}

	/** @deprecated Use cleanup instead. */
	stopPolling(): void {
		this.cleanup();
	}
}

export const ai = new AiStore();
