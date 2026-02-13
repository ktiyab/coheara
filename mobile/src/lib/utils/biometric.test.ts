// M1-01: Biometric gate tests — 5 tests
import { describe, it, expect } from 'vitest';
import { BiometricGate, MockBiometricProvider, NoOpBiometricProvider } from './biometric.js';

describe('biometric gate', () => {
	it('succeeds when biometric is available and user verifies', async () => {
		const provider = new MockBiometricProvider(
			{ available: true, type: 'fingerprint' },
			true
		);
		const gate = new BiometricGate(provider);

		const cap = await gate.checkAvailability();
		expect(cap.available).toBe(true);
		expect(cap.type).toBe('fingerprint');

		const result = await gate.attempt();
		expect(result).toBe('success');
		expect(gate.remainingAttempts).toBe(3);
	});

	it('returns failed when user rejects biometric', async () => {
		const provider = new MockBiometricProvider(
			{ available: true, type: 'face' },
			false
		);
		const gate = new BiometricGate(provider);

		const result = await gate.attempt();
		expect(result).toBe('failed');
		expect(gate.remainingAttempts).toBe(2);
	});

	it('returns unavailable when device lacks biometric', async () => {
		const provider = new NoOpBiometricProvider();
		const gate = new BiometricGate(provider);

		const cap = await gate.checkAvailability();
		expect(cap.available).toBe(false);

		const result = await gate.attempt();
		expect(result).toBe('unavailable');
	});

	it('locks out after 3 consecutive failures', async () => {
		const provider = new MockBiometricProvider(
			{ available: true, type: 'fingerprint' },
			false
		);
		const gate = new BiometricGate(provider, 3);

		expect(gate.remainingAttempts).toBe(3);

		const r1 = await gate.attempt();
		expect(r1).toBe('failed');
		expect(gate.remainingAttempts).toBe(2);

		const r2 = await gate.attempt();
		expect(r2).toBe('failed');
		expect(gate.remainingAttempts).toBe(1);

		const r3 = await gate.attempt();
		expect(r3).toBe('locked_out');
		expect(gate.remainingAttempts).toBe(0);

		// Further attempts also return locked_out
		const r4 = await gate.attempt();
		expect(r4).toBe('locked_out');
	});

	it('resets attempt counter after explicit reset', async () => {
		const provider = new MockBiometricProvider(
			{ available: true, type: 'fingerprint' },
			false
		);
		const gate = new BiometricGate(provider, 3);

		await gate.attempt(); // fail 1
		await gate.attempt(); // fail 2
		expect(gate.remainingAttempts).toBe(1);

		gate.reset();
		expect(gate.remainingAttempts).toBe(3);

		// Can attempt again
		provider.setSuccess(true);
		const result = await gate.attempt();
		expect(result).toBe('success');
	});
});
