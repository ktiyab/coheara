// M1-01: Session management — timeout, auth state, biometric gating
import { writable, derived, get } from 'svelte/store';
import type { AuthState, SessionConfig } from '$lib/types/index.js';

/** Default session timeout: 5 minutes (Nadia requirement) */
export const SESSION_TIMEOUT_MS = 5 * 60 * 1000;

/** Maximum failed biometric attempts before lockout */
export const MAX_BIOMETRIC_ATTEMPTS = 3;

/** Lockout cooldown: 30 seconds */
export const LOCKOUT_COOLDOWN_MS = 30 * 1000;

/** Core auth state store */
export const authState = writable<AuthState>({ state: 'unauthenticated' });

/** Whether user is fully authenticated */
export const isAuthenticated = derived(authState, ($a) =>
	$a.state === 'authenticated'
);

/** Whether biometric is required to proceed */
export const requiresBiometric = derived(authState, ($a) =>
	$a.state === 'locked' || $a.state === 'unauthenticated'
);

/** Session config store */
export const sessionConfig = writable<SessionConfig>({
	timeoutMs: SESSION_TIMEOUT_MS,
	biometricEnabled: false
});

/** Failed attempt counter */
const failedAttempts = writable(0);

/** Background timer handle */
let backgroundTimer: ReturnType<typeof setTimeout> | null = null;

/** Timestamp when app went to background */
let backgroundStartTime: number | null = null;

/** Authenticate successfully with cache key */
export function authenticate(cacheKey: string): void {
	failedAttempts.set(0);
	authState.set({ state: 'authenticated', cacheKey });
}

/** Record a failed biometric attempt; lockout after MAX_BIOMETRIC_ATTEMPTS */
export function recordFailedAttempt(): void {
	failedAttempts.update((n) => {
		const next = n + 1;
		if (next >= MAX_BIOMETRIC_ATTEMPTS) {
			authState.set({
				state: 'locked_out',
				attemptsRemaining: 0,
				cooldownUntil: Date.now() + LOCKOUT_COOLDOWN_MS
			});
		}
		return next;
	});
}

/** Get current failed attempt count */
export function getFailedAttempts(): number {
	return get(failedAttempts);
}

/** Lock the session (biometric required to re-enter) */
export function lockSession(): void {
	failedAttempts.set(0);
	authState.set({ state: 'locked' });
}

/** Reset auth to unauthenticated */
export function resetAuth(): void {
	failedAttempts.set(0);
	clearBackgroundTimer();
	authState.set({ state: 'unauthenticated' });
}

/** Called when app goes to background — starts timeout timer */
export function onBackground(): void {
	const config = get(sessionConfig);
	backgroundStartTime = Date.now();

	clearBackgroundTimer();
	backgroundTimer = setTimeout(() => {
		lockSession();
		backgroundTimer = null;
	}, config.timeoutMs);
}

/** Called when app returns to foreground — check if timed out */
export function onForeground(): boolean {
	clearBackgroundTimer();

	if (backgroundStartTime === null) return false;

	const config = get(sessionConfig);
	const elapsed = Date.now() - backgroundStartTime;
	backgroundStartTime = null;

	if (elapsed >= config.timeoutMs) {
		lockSession();
		return true; // session expired
	}

	return false; // session still valid
}

/** Check if lockout has expired and reset if so */
export function checkLockoutExpiry(): boolean {
	const current = get(authState);
	if (current.state === 'locked_out' && Date.now() >= current.cooldownUntil) {
		failedAttempts.set(0);
		authState.set({ state: 'locked' });
		return true;
	}
	return false;
}

function clearBackgroundTimer(): void {
	if (backgroundTimer !== null) {
		clearTimeout(backgroundTimer);
		backgroundTimer = null;
	}
	backgroundStartTime = null;
}

/** Reset all session state (for testing) */
export function resetSessionState(): void {
	clearBackgroundTimer();
	failedAttempts.set(0);
	authState.set({ state: 'unauthenticated' });
	sessionConfig.set({ timeoutMs: SESSION_TIMEOUT_MS, biometricEnabled: false });
}
