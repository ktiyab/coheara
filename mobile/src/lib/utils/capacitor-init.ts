// Capacitor provider initialization — wires native providers on app start
//
// Imported from +layout.svelte to initialize all Capacitor providers
// before the app renders. Falls back to no-op/mock providers on web.
import { Capacitor } from '@capacitor/core';
import { initSecureStorage } from './secure-storage.js';

/** Whether the app is running on a native platform (iOS/Android) */
export const isNative = Capacitor.isNativePlatform();

/**
 * Initialize all Capacitor native providers.
 * Call once at app startup (in +layout.svelte or +layout.ts).
 *
 * On web, this is a no-op — default providers (NoOp/Memory) are already active.
 */
export async function initCapacitorProviders(): Promise<void> {
	if (!isNative) return;

	// Secure storage — switch from in-memory to Capacitor Preferences
	const { CapacitorSecureStorageProvider } = await import('./capacitor-secure-storage.js');
	initSecureStorage(new CapacitorSecureStorageProvider());
}

/**
 * Create the appropriate BiometricProvider for the current platform.
 * Lazy-imported to avoid bundling native deps in web builds.
 */
export async function createBiometricProvider() {
	if (!isNative) {
		const { NoOpBiometricProvider } = await import('./biometric.js');
		return new NoOpBiometricProvider();
	}
	const { CapacitorBiometricProvider } = await import('./capacitor-biometric.js');
	return new CapacitorBiometricProvider();
}

/**
 * Create the appropriate LifecycleListener for the current platform.
 */
export async function createLifecycleListener() {
	if (!isNative) {
		const { NoOpLifecycleListener } = await import('./lifecycle.js');
		return new NoOpLifecycleListener();
	}
	const { CapacitorLifecycleListener } = await import('./capacitor-lifecycle.js');
	return new CapacitorLifecycleListener();
}

/**
 * Create the appropriate ScreenshotProvider for the current platform.
 */
export async function createScreenshotProvider() {
	if (!isNative) {
		const { NoOpScreenshotProvider } = await import('./screenshot.js');
		return new NoOpScreenshotProvider();
	}
	const { CapacitorScreenshotProvider } = await import('./capacitor-screenshot.js');
	return new CapacitorScreenshotProvider();
}

/**
 * Create the appropriate DeviceIntegrityProvider for the current platform.
 */
export async function createDeviceIntegrityProvider() {
	if (!isNative) {
		const { CleanDeviceProvider } = await import('./device-integrity.js');
		return new CleanDeviceProvider();
	}
	const { CapacitorDeviceIntegrityProvider } = await import('./capacitor-device-integrity.js');
	return new CapacitorDeviceIntegrityProvider();
}
