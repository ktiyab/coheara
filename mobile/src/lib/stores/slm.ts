// M2-01: SLM Store — model lifecycle state machine, download tracking, inference management
import { writable, derived, get } from 'svelte/store';
import type {
	ModelState,
	ModelChoice,
	ModelInfo,
	DownloadProgress,
	SlmResult,
	FinishReason,
	GenerateOptions
} from '$lib/types/slm.js';
import {
	MODEL_SPECS,
	DEFAULT_MODEL,
	DEFAULT_GENERATE_OPTIONS,
	MIN_RAM_BYTES,
	BACKGROUND_UNLOAD_MS
} from '$lib/types/slm.js';
import type { ModelChoice as ModelChoiceType } from '$lib/types/slm.js';

// === STATE ===

export const modelState = writable<ModelState>('not_downloaded');
export const selectedModel = writable<ModelChoice>(DEFAULT_MODEL);
export const downloadProgress = writable<DownloadProgress | null>(null);
export const modelInfo = writable<ModelInfo | null>(null);
export const lastGenerationResult = writable<SlmResult | null>(null);

// === DERIVED ===

export const isModelReady = derived(modelState, ($s) => $s === 'ready');
export const isModelGenerating = derived(modelState, ($s) => $s === 'generating');
export const isModelDownloaded = derived(modelState, ($s) =>
	$s === 'downloaded' || $s === 'loading' || $s === 'ready' || $s === 'generating'
);
export const isDownloading = derived(modelState, ($s) => $s === 'downloading');
export const canGenerate = derived(modelState, ($s) => $s === 'ready');

export const selectedModelSpec = derived(selectedModel, ($id) => MODEL_SPECS[$id]);

// === DEVICE CAPABILITY ===

let deviceRamBytes = 8 * 1024 * 1024 * 1024; // Default 8GB (for testing)
let deviceFreeStorageBytes = 10 * 1024 * 1024 * 1024;

export function setDeviceCapabilities(ramBytes: number, freeStorageBytes: number): void {
	deviceRamBytes = ramBytes;
	deviceFreeStorageBytes = freeStorageBytes;

	if (ramBytes < MIN_RAM_BYTES) {
		modelState.set('not_capable');
	}
}

export function isDeviceCapable(): boolean {
	return deviceRamBytes >= MIN_RAM_BYTES;
}

export function hasEnoughStorage(modelId: ModelChoice): boolean {
	return deviceFreeStorageBytes >= MODEL_SPECS[modelId].sizeBytes;
}

export function getDeviceRam(): number {
	return deviceRamBytes;
}

// === SLM DISCOVERY CRITERIA ===

let chatSessionCount = 0;
let offlineChatAttempts = 0;

export function incrementChatSessions(): void {
	chatSessionCount++;
}

export function incrementOfflineChatAttempts(): void {
	offlineChatAttempts++;
}

export function shouldShowSlmPromotion(): boolean {
	return (
		isDeviceCapable() &&
		hasEnoughStorage(DEFAULT_MODEL) &&
		chatSessionCount >= 3 &&
		offlineChatAttempts >= 1 &&
		get(modelState) === 'not_downloaded'
	);
}

// === DOWNLOAD LIFECYCLE ===

export function startDownload(modelId: ModelChoice): void {
	if (!isDeviceCapable()) return;
	if (get(modelState) !== 'not_downloaded') return;

	selectedModel.set(modelId);
	modelState.set('downloading');
	downloadProgress.set({ percent: 0, downloadedBytes: 0, totalBytes: MODEL_SPECS[modelId].sizeBytes });
}

export function updateDownloadProgress(percent: number, downloadedBytes: number): void {
	const totalBytes = MODEL_SPECS[get(selectedModel)].sizeBytes;
	downloadProgress.set({ percent: Math.min(100, Math.max(0, percent)), downloadedBytes, totalBytes });
}

export function completeDownload(): void {
	if (get(modelState) !== 'downloading') return;

	const model = get(selectedModel);
	const spec = MODEL_SPECS[model];

	modelState.set('downloaded');
	downloadProgress.set(null);
	modelInfo.set({
		id: model,
		name: spec.name,
		sizeBytes: spec.sizeBytes,
		quantization: spec.quantization,
		loaded: false,
		lastUsed: null
	});
}

// === SHA-256 CHECKSUM VERIFICATION (RS-M2-01-001) ===

/** Compute SHA-256 hex digest of binary data */
export async function computeSha256(data: BufferSource): Promise<string> {
	const hashBuffer = await crypto.subtle.digest('SHA-256', data);
	const hashArray = Array.from(new Uint8Array(hashBuffer));
	return hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
}

/** Verify data integrity against an expected SHA-256 hex digest */
export async function verifyChecksum(data: BufferSource, expectedHash: string): Promise<boolean> {
	const actual = await computeSha256(data);
	return actual === expectedHash;
}

/** Verify downloaded model file against its expected hash from MODEL_SPECS */
export async function verifyModelChecksum(data: BufferSource, modelId: ModelChoiceType): Promise<boolean> {
	const expected = MODEL_SPECS[modelId].expectedSha256;
	if (!expected) return true; // No expected hash configured yet
	return verifyChecksum(data, expected);
}

/** Complete download with SHA-256 verification. Returns false and cancels if hash mismatches. */
export async function completeDownloadVerified(fileData: BufferSource): Promise<boolean> {
	if (get(modelState) !== 'downloading') return false;

	const model = get(selectedModel);
	const isValid = await verifyModelChecksum(fileData, model);

	if (!isValid) {
		cancelDownload();
		return false;
	}

	completeDownload();
	return true;
}

export function cancelDownload(): void {
	if (get(modelState) !== 'downloading') return;
	modelState.set('not_downloaded');
	downloadProgress.set(null);
}

// === MODEL LOAD/UNLOAD ===

export function loadModel(): void {
	const state = get(modelState);
	if (state !== 'downloaded') return;

	modelState.set('loading');
	// In production: native plugin loads GGUF via mmap
	// Simulated: instant transition to ready
	modelState.set('ready');
	modelInfo.update(($i) => $i ? { ...$i, loaded: true } : $i);
}

export function unloadModel(): void {
	const state = get(modelState);
	if (state !== 'ready' && state !== 'generating') return;

	modelState.set('downloaded');
	modelInfo.update(($i) => $i ? { ...$i, loaded: false } : $i);
}

// === GENERATION ===

export function startGeneration(): boolean {
	if (get(modelState) !== 'ready') return false;
	modelState.set('generating');
	return true;
}

export function completeGeneration(tokensGenerated: number, timeMs: number, finishReason: FinishReason): void {
	const result: SlmResult = {
		tokensGenerated,
		timeMs,
		tokensPerSecond: timeMs > 0 ? (tokensGenerated / timeMs) * 1000 : 0,
		finishReason
	};

	lastGenerationResult.set(result);
	modelState.set('ready');
	modelInfo.update(($i) => $i ? { ...$i, lastUsed: new Date().toISOString() } : $i);
}

export function abortGeneration(): void {
	if (get(modelState) !== 'generating') return;
	modelState.set('ready');
}

// === BACKGROUND UNLOAD TIMER (RS-M2-01-002) ===

let backgroundTimer: ReturnType<typeof setTimeout> | null = null;

/** Called when app enters background. Starts 30s countdown to unload model from RAM. */
export function onAppBackground(): void {
	const state = get(modelState);
	if (state === 'ready' || state === 'generating') {
		backgroundTimer = setTimeout(() => {
			unloadModel();
			backgroundTimer = null;
		}, BACKGROUND_UNLOAD_MS);
	}
}

/** Called when app returns to foreground. Cancels pending unload timer. */
export function onAppForeground(): void {
	if (backgroundTimer !== null) {
		clearTimeout(backgroundTimer);
		backgroundTimer = null;
	}
}

// === DELETE ===

export function deleteModel(): void {
	const state = get(modelState);
	if (state === 'not_capable' || state === 'not_downloaded') return;

	// Unload if loaded
	if (state === 'ready' || state === 'generating' || state === 'loading') {
		unloadModel();
	}

	modelState.set('not_downloaded');
	modelInfo.set(null);
	downloadProgress.set(null);
}

// === RESET (for tests) ===

export function resetSlmState(): void {
	if (backgroundTimer !== null) {
		clearTimeout(backgroundTimer);
		backgroundTimer = null;
	}
	deviceRamBytes = 8 * 1024 * 1024 * 1024;
	deviceFreeStorageBytes = 10 * 1024 * 1024 * 1024;
	chatSessionCount = 0;
	offlineChatAttempts = 0;
	modelState.set('not_downloaded');
	selectedModel.set(DEFAULT_MODEL);
	downloadProgress.set(null);
	modelInfo.set(null);
	lastGenerationResult.set(null);
}
