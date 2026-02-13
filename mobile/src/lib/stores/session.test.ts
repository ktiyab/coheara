// M1-01: Session management tests — 4 tests
import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { get } from 'svelte/store';
import {
	authState,
	isAuthenticated,
	requiresBiometric,
	sessionConfig,
	authenticate,
	recordFailedAttempt,
	getFailedAttempts,
	lockSession,
	resetAuth,
	onBackground,
	onForeground,
	checkLockoutExpiry,
	resetSessionState,
	SESSION_TIMEOUT_MS,
	MAX_BIOMETRIC_ATTEMPTS,
	LOCKOUT_COOLDOWN_MS
} from './session.js';

describe('session store', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		resetSessionState();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('starts unauthenticated and authenticates with cache key', () => {
		expect(get(authState).state).toBe('unauthenticated');
		expect(get(isAuthenticated)).toBe(false);
		expect(get(requiresBiometric)).toBe(true);

		authenticate('test-cache-key-123');

		expect(get(authState).state).toBe('authenticated');
		expect(get(isAuthenticated)).toBe(true);
		expect(get(requiresBiometric)).toBe(false);

		const state = get(authState);
		if (state.state === 'authenticated') {
			expect(state.cacheKey).toBe('test-cache-key-123');
		}
	});

	it('locks session after background timeout expires', () => {
		authenticate('key');
		sessionConfig.set({ timeoutMs: 1000, biometricEnabled: true });

		onBackground();
		expect(get(authState).state).toBe('authenticated');

		// Advance past timeout
		vi.advanceTimersByTime(1100);
		expect(get(authState).state).toBe('locked');
		expect(get(requiresBiometric)).toBe(true);
	});

	it('does not lock if returning before timeout', () => {
		authenticate('key');
		sessionConfig.set({ timeoutMs: 5000, biometricEnabled: true });

		onBackground();

		// Return before timeout
		vi.advanceTimersByTime(2000);
		const expired = onForeground();

		expect(expired).toBe(false);
		expect(get(authState).state).toBe('authenticated');
	});

	it('locks out after max failed biometric attempts then recovers', () => {
		lockSession(); // Start locked

		// Fail 3 times
		recordFailedAttempt();
		expect(getFailedAttempts()).toBe(1);
		expect(get(authState).state).toBe('locked');

		recordFailedAttempt();
		expect(getFailedAttempts()).toBe(2);
		expect(get(authState).state).toBe('locked');

		recordFailedAttempt(); // Third attempt → lockout
		expect(getFailedAttempts()).toBe(3);
		expect(get(authState).state).toBe('locked_out');

		// Lockout hasn't expired yet
		expect(checkLockoutExpiry()).toBe(false);

		// Advance past cooldown
		vi.advanceTimersByTime(LOCKOUT_COOLDOWN_MS + 100);
		expect(checkLockoutExpiry()).toBe(true);
		expect(get(authState).state).toBe('locked'); // Back to locked, can retry
	});
});
