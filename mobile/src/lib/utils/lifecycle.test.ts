// M1-01: App lifecycle tests — 6 tests
import { describe, it, expect, beforeEach } from 'vitest';
import { handleColdStart, handleWarmStart, handleNetworkChange } from './lifecycle.js';
import { MockBiometricProvider, NoOpBiometricProvider } from './biometric.js';
import { MemorySecureStorage, initSecureStorage, STORAGE_KEYS } from './secure-storage.js';

describe('app lifecycle', () => {
	let storage: MemorySecureStorage;

	beforeEach(() => {
		storage = new MemorySecureStorage();
		initSecureStorage(storage);
	});

	it('cold start unpaired → shows pairing screen', async () => {
		// No session token stored
		const provider = new NoOpBiometricProvider();
		const result = await handleColdStart(provider);
		expect(result.screen).toBe('pairing');
	});

	it('cold start paired + biometric enabled → shows biometric gate', async () => {
		await storage.set(STORAGE_KEYS.SESSION_TOKEN, 'test-token');
		await storage.set(STORAGE_KEYS.BIOMETRIC_ENABLED, 'true');
		await storage.set(STORAGE_KEYS.CACHE_KEY, 'cache-key-123');

		const provider = new MockBiometricProvider(
			{ available: true, type: 'fingerprint' },
			true
		);

		const result = await handleColdStart(provider);
		expect(result.screen).toBe('biometric_gate');
	});

	it('cold start paired + no biometric → goes directly to home', async () => {
		await storage.set(STORAGE_KEYS.SESSION_TOKEN, 'test-token');
		await storage.set(STORAGE_KEYS.BIOMETRIC_ENABLED, 'false');
		await storage.set(STORAGE_KEYS.CACHE_KEY, 'cache-key-123');

		const provider = new NoOpBiometricProvider();
		const result = await handleColdStart(provider);

		expect(result.screen).toBe('home');
		if (result.screen === 'home') {
			expect(result.cacheKey).toBe('cache-key-123');
		}
	});

	it('cold start paired but missing cache key → shows error', async () => {
		await storage.set(STORAGE_KEYS.SESSION_TOKEN, 'test-token');
		await storage.set(STORAGE_KEYS.BIOMETRIC_ENABLED, 'false');
		// No cache key set

		const provider = new NoOpBiometricProvider();
		const result = await handleColdStart(provider);

		expect(result.screen).toBe('error');
		if (result.screen === 'error') {
			expect(result.message).toContain('re-pair');
		}
	});

	it('warm start with expired session + biometric → requires biometric', () => {
		const result = handleWarmStart(true, true);
		expect(result.action).toBe('biometric_gate');
	});

	it('warm start with expired session + no biometric → resumes (Mamadou compromise)', () => {
		// Mamadou compromise: session continues on next open without biometric
		const result = handleWarmStart(true, false);
		expect(result.action).toBe('resume');
	});
});

describe('network change', () => {
	let storage: MemorySecureStorage;

	beforeEach(() => {
		storage = new MemorySecureStorage();
		initSecureStorage(storage);
	});

	it('attempts reconnection when network available and paired', async () => {
		await storage.set(STORAGE_KEYS.SESSION_TOKEN, 'test-token');

		const shouldReconnect = await handleNetworkChange(true);
		expect(shouldReconnect).toBe(true);
	});

	it('does not reconnect when network lost', async () => {
		const shouldReconnect = await handleNetworkChange(false);
		expect(shouldReconnect).toBe(false);
	});
});
