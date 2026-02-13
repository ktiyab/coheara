// IMP-009: CapacitorSecureStorageProvider — wraps @capacitor/preferences for persistence
//
// Note: @capacitor/preferences uses SharedPreferences (Android) / UserDefaults (iOS).
// For truly sensitive values (session tokens, cache keys), the biometric-secured
// credential storage in @capgo/capacitor-native-biometric is used via the
// BiometricProvider. Preferences handles the bulk key/value storage.
import { Preferences } from '@capacitor/preferences';
import type { SecureStorageProvider } from './secure-storage.js';

/** Capacitor implementation using @capacitor/preferences */
export class CapacitorSecureStorageProvider implements SecureStorageProvider {
	async get(key: string): Promise<string | null> {
		const result = await Preferences.get({ key });
		return result.value;
	}

	async set(key: string, value: string): Promise<void> {
		await Preferences.set({ key, value });
	}

	async remove(key: string): Promise<void> {
		await Preferences.remove({ key });
	}
}
