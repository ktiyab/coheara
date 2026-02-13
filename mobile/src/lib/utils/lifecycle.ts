// M1-01: App lifecycle handler — cold start, warm start, background, foreground
import type { AppLifecycleState, BiometricCapability } from '$lib/types/index.js';
import type { BiometricProvider } from './biometric.js';
import { isPaired, isBiometricEnabled, secureGet, STORAGE_KEYS } from './secure-storage.js';

/** Lifecycle event listener interface (for native Capacitor App plugin) */
export interface LifecycleListener {
	onForeground(callback: () => void): void;
	onBackground(callback: () => void): void;
	onNetworkChange(callback: (connected: boolean) => void): void;
}

/** Result of the cold start check — determines which screen to show */
export type ColdStartResult =
	| { screen: 'pairing' }
	| { screen: 'biometric_gate' }
	| { screen: 'home'; cacheKey: string }
	| { screen: 'error'; message: string };

/** Result of the warm start check */
export type WarmStartResult =
	| { action: 'resume' }
	| { action: 'biometric_gate' }
	| { action: 'reconnect' };

/**
 * Determine the app's initial screen on cold start.
 *
 * Flow:
 * 1. Not paired → pairing screen
 * 2. Paired + biometric enabled → biometric gate
 * 3. Paired + no biometric → load cache key → home
 */
export async function handleColdStart(
	biometricProvider: BiometricProvider
): Promise<ColdStartResult> {
	const paired = await isPaired();
	if (!paired) {
		return { screen: 'pairing' };
	}

	const bioEnabled = await isBiometricEnabled();
	if (bioEnabled) {
		const cap = await biometricProvider.isAvailable();
		if (cap.available) {
			return { screen: 'biometric_gate' };
		}
		// Biometric enabled but not available (device changed?) — fallback to home
	}

	const cacheKey = await secureGet(STORAGE_KEYS.CACHE_KEY);
	if (!cacheKey) {
		return { screen: 'error', message: 'Cache key not found. Please re-pair your device.' };
	}

	return { screen: 'home', cacheKey };
}

/**
 * Determine action on warm start (app returning from background).
 *
 * @param sessionExpired - Whether the session timeout has expired
 * @param biometricEnabled - Whether biometric gate is active
 */
export function handleWarmStart(
	sessionExpired: boolean,
	biometricEnabled: boolean
): WarmStartResult {
	if (sessionExpired && biometricEnabled) {
		return { action: 'biometric_gate' };
	}

	if (sessionExpired && !biometricEnabled) {
		// Mamadou compromise: session continues on next open (no biometric device)
		return { action: 'resume' };
	}

	return { action: 'resume' };
}

/**
 * Handle network change — attempt reconnection to desktop.
 * Returns true if reconnection should be attempted.
 */
export async function handleNetworkChange(connected: boolean): Promise<boolean> {
	if (!connected) return false;

	const paired = await isPaired();
	return paired;
}

/** No-op lifecycle listener for non-native environments */
export class NoOpLifecycleListener implements LifecycleListener {
	onForeground(_callback: () => void): void { /* no-op */ }
	onBackground(_callback: () => void): void { /* no-op */ }
	onNetworkChange(_callback: (connected: boolean) => void): void { /* no-op */ }
}
