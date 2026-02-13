// M1-01: Biometric authentication — recommended not mandatory (Mamadou compromise)
import type { BiometricCapability } from '$lib/types/index.js';

/** Provider interface for native biometric APIs (mockable in tests) */
export interface BiometricProvider {
	isAvailable(): Promise<BiometricCapability>;
	verify(reason: string): Promise<boolean>;
}

/** Default no-op provider when no native API is available */
export class NoOpBiometricProvider implements BiometricProvider {
	async isAvailable(): Promise<BiometricCapability> {
		return { available: false, type: 'none' };
	}
	async verify(_reason: string): Promise<boolean> {
		return false;
	}
}

/** In-memory provider for testing */
export class MockBiometricProvider implements BiometricProvider {
	private capability: BiometricCapability;
	private shouldSucceed: boolean;

	constructor(capability: BiometricCapability, shouldSucceed = true) {
		this.capability = capability;
		this.shouldSucceed = shouldSucceed;
	}

	async isAvailable(): Promise<BiometricCapability> {
		return this.capability;
	}

	async verify(_reason: string): Promise<boolean> {
		return this.shouldSucceed;
	}

	setSuccess(value: boolean): void {
		this.shouldSucceed = value;
	}
}

/** Biometric gate logic — manages the auth check flow */
export class BiometricGate {
	private provider: BiometricProvider;
	private maxAttempts: number;
	private attempts: number;

	constructor(provider: BiometricProvider, maxAttempts = 3) {
		this.provider = provider;
		this.maxAttempts = maxAttempts;
		this.attempts = 0;
	}

	/** Check if biometric is available on this device */
	async checkAvailability(): Promise<BiometricCapability> {
		return this.provider.isAvailable();
	}

	/**
	 * Attempt biometric verification.
	 * Returns: 'success' | 'failed' | 'locked_out' | 'unavailable'
	 */
	async attempt(): Promise<'success' | 'failed' | 'locked_out' | 'unavailable'> {
		const cap = await this.provider.isAvailable();
		if (!cap.available) return 'unavailable';

		if (this.attempts >= this.maxAttempts) return 'locked_out';

		const verified = await this.provider.verify('Unlock Coheara');
		if (verified) {
			this.attempts = 0;
			return 'success';
		}

		this.attempts++;
		if (this.attempts >= this.maxAttempts) return 'locked_out';
		return 'failed';
	}

	/** Get remaining attempts before lockout */
	get remainingAttempts(): number {
		return Math.max(0, this.maxAttempts - this.attempts);
	}

	/** Reset attempt counter (e.g., after cooldown) */
	reset(): void {
		this.attempts = 0;
	}
}
