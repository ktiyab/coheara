/**
 * L6-01/L6-02/L6-04: AI model management API wrappers.
 *
 * Tauri invoke wrappers for all L6 AI Engine IPC commands.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
	ModelInfo,
	ModelDetail,
	OllamaHealth,
	ResolvedModel,
	ModelPullProgress,
	HardwareStatus,
	CapabilityTag
} from '$lib/types/ai';

// ── L6-01: Ollama Integration ──────────────────────────────

export async function ollamaHealthCheck(): Promise<OllamaHealth> {
	return invoke<OllamaHealth>('ollama_health_check');
}

export async function listOllamaModels(): Promise<ModelInfo[]> {
	return invoke<ModelInfo[]>('list_ollama_models');
}

export async function showOllamaModel(name: string): Promise<ModelDetail> {
	return invoke<ModelDetail>('show_ollama_model', { name });
}

export async function pullOllamaModel(name: string): Promise<void> {
	return invoke<void>('pull_ollama_model', { name });
}

export async function cancelModelPull(): Promise<void> {
	return invoke<void>('cancel_model_pull');
}

export async function deleteOllamaModel(name: string): Promise<void> {
	return invoke<void>('delete_ollama_model', { name });
}

// ── L6-04: Model Preferences ──────────────────────────────

export async function setActiveModel(
	modelName: string,
	source?: 'user' | 'wizard'
): Promise<ResolvedModel> {
	return invoke<ResolvedModel>('set_active_model', {
		modelName,
		source: source ?? 'user'
	});
}

export async function getActiveModel(): Promise<ResolvedModel | null> {
	return invoke<ResolvedModel | null>('get_active_model');
}

export async function clearActiveModel(): Promise<void> {
	return invoke<void>('clear_active_model');
}

export async function setUserPreference(key: string, value: string): Promise<void> {
	return invoke<void>('set_user_preference_cmd', { key, value });
}

export async function getUserPreference(key: string): Promise<string | null> {
	return invoke<string | null>('get_user_preference_cmd', { key });
}

// ── CT-01: Model Capability Tags + Enabled Flag ──────────

export async function getModelTags(modelName: string): Promise<CapabilityTag[]> {
	return invoke<CapabilityTag[]>('get_model_tags', { modelName });
}

export async function setModelTags(modelName: string, tags: CapabilityTag[]): Promise<void> {
	return invoke<void>('set_model_tags', { modelName, tags });
}

export async function addModelTag(modelName: string, tag: CapabilityTag): Promise<void> {
	return invoke<void>('add_model_tag', { modelName, tag });
}

export async function removeModelTag(modelName: string, tag: CapabilityTag): Promise<void> {
	return invoke<void>('remove_model_tag', { modelName, tag });
}

export async function getAllModelTags(): Promise<Record<string, CapabilityTag[]>> {
	return invoke<Record<string, CapabilityTag[]>>('get_all_model_tags');
}

export async function isModelEnabled(modelName: string): Promise<boolean> {
	return invoke<boolean>('is_model_enabled', { modelName });
}

export async function setModelEnabled(modelName: string, enabled: boolean): Promise<void> {
	return invoke<void>('set_model_enabled', { modelName, enabled });
}

export async function getDisabledModels(): Promise<string[]> {
	return invoke<string[]>('get_disabled_models');
}

// ── L6-03: AI Setup Wizard ────────────────────────────────

export async function verifyAiModel(modelName: string): Promise<boolean> {
	return invoke<boolean>('verify_ai_model', { modelName });
}

// ── T7: Hardware Detection ────────────────────────────────

export async function getHardwareProfile(): Promise<HardwareStatus> {
	return invoke<HardwareStatus>('get_hardware_profile');
}

// ── Tauri Events ───────────────────────────────────────────

export function onPullProgress(
	callback: (progress: ModelPullProgress) => void
): Promise<() => void> {
	return listen<ModelPullProgress>('model-pull-progress', (event) => {
		callback(event.payload);
	});
}
