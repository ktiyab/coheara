// M1-01: Secure storage — Keychain (iOS) / Keystore (Android) wrapper

/** Provider interface for native secure storage (mockable in tests) */
export interface SecureStorageProvider {
	get(key: string): Promise<string | null>;
	set(key: string, value: string): Promise<void>;
	remove(key: string): Promise<void>;
}

/** Well-known storage keys */
export const STORAGE_KEYS = {
	SESSION_TOKEN: 'session_token',
	CACHE_KEY: 'cache_key',
	BIOMETRIC_ENABLED: 'biometric_enabled',
	DESKTOP_URL: 'desktop_url',
	DEVICE_ID: 'device_id',
	LAST_PROFILE: 'last_profile',
	EMERGENCY_CARD_CONFIG: 'emergency_card_config',
	PAIRING_CERT: 'pairing_cert'
} as const;

/** In-memory implementation for testing */
export class MemorySecureStorage implements SecureStorageProvider {
	private store = new Map<string, string>();

	async get(key: string): Promise<string | null> {
		return this.store.get(key) ?? null;
	}

	async set(key: string, value: string): Promise<void> {
		this.store.set(key, value);
	}

	async remove(key: string): Promise<void> {
		this.store.delete(key);
	}

	clear(): void {
		this.store.clear();
	}

	has(key: string): boolean {
		return this.store.has(key);
	}
}

/** Active storage provider (set during app initialization) */
let activeProvider: SecureStorageProvider = new MemorySecureStorage();

/** Initialize with a native provider */
export function initSecureStorage(provider: SecureStorageProvider): void {
	activeProvider = provider;
}

/** Get a value from secure storage */
export async function secureGet(key: string): Promise<string | null> {
	return activeProvider.get(key);
}

/** Set a value in secure storage */
export async function secureSet(key: string, value: string): Promise<void> {
	return activeProvider.set(key, value);
}

/** Remove a value from secure storage */
export async function secureRemove(key: string): Promise<void> {
	return activeProvider.remove(key);
}

/** Check if the device is paired (has a session token) */
export async function isPaired(): Promise<boolean> {
	const token = await secureGet(STORAGE_KEYS.SESSION_TOKEN);
	return token !== null;
}

/** Check if biometric is enabled in preferences */
export async function isBiometricEnabled(): Promise<boolean> {
	const val = await secureGet(STORAGE_KEYS.BIOMETRIC_ENABLED);
	return val === 'true';
}
