// M2-01: SLM Store tests — model lifecycle state machine, device capability, generation
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
	modelState,
	selectedModel,
	downloadProgress,
	modelInfo,
	lastGenerationResult,
	isModelReady,
	isModelGenerating,
	isModelDownloaded,
	isDownloading,
	canGenerate,
	selectedModelSpec,
	setDeviceCapabilities,
	isDeviceCapable,
	hasEnoughStorage,
	getDeviceRam,
	incrementChatSessions,
	incrementOfflineChatAttempts,
	shouldShowSlmPromotion,
	startDownload,
	updateDownloadProgress,
	completeDownload,
	cancelDownload,
	loadModel,
	unloadModel,
	startGeneration,
	completeGeneration,
	abortGeneration,
	deleteModel,
	resetSlmState
} from './slm.js';
import { MODEL_SPECS, MIN_RAM_BYTES } from '$lib/types/slm.js';

beforeEach(() => resetSlmState());

// === DEVICE CAPABILITY ===

describe('slm store — device capability', () => {
	it('defaults to capable (8GB testing default)', () => {
		expect(isDeviceCapable()).toBe(true);
		expect(getDeviceRam()).toBe(8 * 1024 * 1024 * 1024);
	});

	it('marks not_capable for 4GB device', () => {
		setDeviceCapabilities(4 * 1024 * 1024 * 1024, 10 * 1024 * 1024 * 1024);
		expect(isDeviceCapable()).toBe(false);
		expect(get(modelState)).toBe('not_capable');
	});

	it('accepts 6GB device as capable', () => {
		setDeviceCapabilities(6 * 1024 * 1024 * 1024, 10 * 1024 * 1024 * 1024);
		expect(isDeviceCapable()).toBe(true);
		expect(get(modelState)).toBe('not_downloaded');
	});

	it('checks storage against model size', () => {
		setDeviceCapabilities(8 * 1024 * 1024 * 1024, 1 * 1024 * 1024 * 1024);
		expect(hasEnoughStorage('gemma-2b-q4')).toBe(false);
		setDeviceCapabilities(8 * 1024 * 1024 * 1024, 10 * 1024 * 1024 * 1024);
		expect(hasEnoughStorage('gemma-2b-q4')).toBe(true);
	});
});

// === DISCOVERY CRITERIA ===

describe('slm store — discovery promotion', () => {
	it('does not show promotion initially', () => {
		expect(shouldShowSlmPromotion()).toBe(false);
	});

	it('shows promotion when all criteria met', () => {
		for (let i = 0; i < 3; i++) incrementChatSessions();
		incrementOfflineChatAttempts();
		expect(shouldShowSlmPromotion()).toBe(true);
	});

	it('does not show promotion with only 2 sessions', () => {
		incrementChatSessions();
		incrementChatSessions();
		incrementOfflineChatAttempts();
		expect(shouldShowSlmPromotion()).toBe(false);
	});

	it('does not show promotion without offline attempt', () => {
		for (let i = 0; i < 3; i++) incrementChatSessions();
		expect(shouldShowSlmPromotion()).toBe(false);
	});

	it('does not show promotion on incapable device', () => {
		setDeviceCapabilities(4 * 1024 * 1024 * 1024, 10 * 1024 * 1024 * 1024);
		for (let i = 0; i < 3; i++) incrementChatSessions();
		incrementOfflineChatAttempts();
		expect(shouldShowSlmPromotion()).toBe(false);
	});
});

// === DOWNLOAD LIFECYCLE ===

describe('slm store — download lifecycle', () => {
	it('starts download and sets progress', () => {
		startDownload('gemma-2b-q4');
		expect(get(modelState)).toBe('downloading');
		expect(get(isDownloading)).toBe(true);
		const prog = get(downloadProgress);
		expect(prog).not.toBeNull();
		expect(prog!.percent).toBe(0);
		expect(prog!.totalBytes).toBe(MODEL_SPECS['gemma-2b-q4'].sizeBytes);
	});

	it('updates download progress with clamping', () => {
		startDownload('gemma-2b-q4');
		updateDownloadProgress(50, 750 * 1024 * 1024);
		expect(get(downloadProgress)!.percent).toBe(50);

		updateDownloadProgress(150, 2000 * 1024 * 1024);
		expect(get(downloadProgress)!.percent).toBe(100);

		updateDownloadProgress(-10, 0);
		expect(get(downloadProgress)!.percent).toBe(0);
	});

	it('completes download and creates model info', () => {
		startDownload('gemma-2b-q4');
		completeDownload();
		expect(get(modelState)).toBe('downloaded');
		expect(get(downloadProgress)).toBeNull();
		const info = get(modelInfo);
		expect(info).not.toBeNull();
		expect(info!.id).toBe('gemma-2b-q4');
		expect(info!.loaded).toBe(false);
		expect(info!.lastUsed).toBeNull();
	});

	it('cancels download and resets state', () => {
		startDownload('gemma-2b-q4');
		cancelDownload();
		expect(get(modelState)).toBe('not_downloaded');
		expect(get(downloadProgress)).toBeNull();
	});

	it('prevents download on incapable device', () => {
		setDeviceCapabilities(4 * 1024 * 1024 * 1024, 10 * 1024 * 1024 * 1024);
		startDownload('gemma-2b-q4');
		expect(get(modelState)).toBe('not_capable');
	});

	it('prevents download when already downloading', () => {
		startDownload('gemma-2b-q4');
		startDownload('phi3-mini-q4');
		expect(get(selectedModel)).toBe('gemma-2b-q4');
	});

	it('selects different model for download', () => {
		startDownload('phi3-mini-q4');
		expect(get(selectedModel)).toBe('phi3-mini-q4');
		expect(get(downloadProgress)!.totalBytes).toBe(MODEL_SPECS['phi3-mini-q4'].sizeBytes);
	});
});

// === MODEL LOAD/UNLOAD ===

describe('slm store — model load/unload', () => {
	it('loads model from downloaded state', () => {
		startDownload('gemma-2b-q4');
		completeDownload();
		loadModel();
		expect(get(modelState)).toBe('ready');
		expect(get(isModelReady)).toBe(true);
		expect(get(modelInfo)!.loaded).toBe(true);
	});

	it('prevents load when not downloaded', () => {
		loadModel();
		expect(get(modelState)).toBe('not_downloaded');
	});

	it('unloads model from ready state', () => {
		startDownload('gemma-2b-q4');
		completeDownload();
		loadModel();
		unloadModel();
		expect(get(modelState)).toBe('downloaded');
		expect(get(isModelReady)).toBe(false);
		expect(get(modelInfo)!.loaded).toBe(false);
	});

	it('prevents unload when not loaded', () => {
		startDownload('gemma-2b-q4');
		completeDownload();
		unloadModel();
		expect(get(modelState)).toBe('downloaded');
	});
});

// === GENERATION ===

describe('slm store — generation', () => {
	function setupReady(): void {
		startDownload('gemma-2b-q4');
		completeDownload();
		loadModel();
	}

	it('starts generation from ready state', () => {
		setupReady();
		const started = startGeneration();
		expect(started).toBe(true);
		expect(get(modelState)).toBe('generating');
		expect(get(isModelGenerating)).toBe(true);
	});

	it('rejects generation when not ready', () => {
		const started = startGeneration();
		expect(started).toBe(false);
		expect(get(modelState)).toBe('not_downloaded');
	});

	it('completes generation with metrics', () => {
		setupReady();
		startGeneration();
		completeGeneration(100, 2000, 'stop');
		expect(get(modelState)).toBe('ready');
		const result = get(lastGenerationResult);
		expect(result).not.toBeNull();
		expect(result!.tokensGenerated).toBe(100);
		expect(result!.timeMs).toBe(2000);
		expect(result!.tokensPerSecond).toBe(50);
		expect(result!.finishReason).toBe('stop');
	});

	it('handles zero timeMs gracefully', () => {
		setupReady();
		startGeneration();
		completeGeneration(10, 0, 'stop');
		expect(get(lastGenerationResult)!.tokensPerSecond).toBe(0);
	});

	it('records max_tokens finish reason', () => {
		setupReady();
		startGeneration();
		completeGeneration(512, 5000, 'max_tokens');
		expect(get(lastGenerationResult)!.finishReason).toBe('max_tokens');
	});

	it('aborts generation and returns to ready', () => {
		setupReady();
		startGeneration();
		abortGeneration();
		expect(get(modelState)).toBe('ready');
	});

	it('updates lastUsed on generation complete', () => {
		setupReady();
		startGeneration();
		completeGeneration(100, 2000, 'stop');
		expect(get(modelInfo)!.lastUsed).not.toBeNull();
	});
});

// === DELETE ===

describe('slm store — model deletion', () => {
	it('deletes downloaded model', () => {
		startDownload('gemma-2b-q4');
		completeDownload();
		deleteModel();
		expect(get(modelState)).toBe('not_downloaded');
		expect(get(modelInfo)).toBeNull();
	});

	it('unloads and deletes ready model', () => {
		startDownload('gemma-2b-q4');
		completeDownload();
		loadModel();
		deleteModel();
		expect(get(modelState)).toBe('not_downloaded');
		expect(get(modelInfo)).toBeNull();
	});

	it('ignores delete on not_downloaded', () => {
		deleteModel();
		expect(get(modelState)).toBe('not_downloaded');
	});

	it('ignores delete on not_capable', () => {
		setDeviceCapabilities(4 * 1024 * 1024 * 1024, 10 * 1024 * 1024 * 1024);
		deleteModel();
		expect(get(modelState)).toBe('not_capable');
	});
});

// === DERIVED STORES ===

describe('slm store — derived stores', () => {
	it('isModelDownloaded is true for downloaded+loading+ready+generating', () => {
		expect(get(isModelDownloaded)).toBe(false);

		startDownload('gemma-2b-q4');
		expect(get(isModelDownloaded)).toBe(false);

		completeDownload();
		expect(get(isModelDownloaded)).toBe(true);

		loadModel();
		expect(get(isModelDownloaded)).toBe(true);

		startGeneration();
		expect(get(isModelDownloaded)).toBe(true);
	});

	it('selectedModelSpec reflects current selection', () => {
		expect(get(selectedModelSpec).id).toBe('gemma-2b-q4');
		startDownload('phi3-mini-q4');
		expect(get(selectedModelSpec).id).toBe('phi3-mini-q4');
	});

	it('canGenerate only true when ready', () => {
		expect(get(canGenerate)).toBe(false);
		startDownload('gemma-2b-q4');
		completeDownload();
		expect(get(canGenerate)).toBe(false);
		loadModel();
		expect(get(canGenerate)).toBe(true);
		startGeneration();
		expect(get(canGenerate)).toBe(false);
	});
});
