// M1-01: Device integrity tests — 2 tests
import { describe, it, expect } from 'vitest';
import {
	checkDeviceIntegrity,
	createIntegrityAuditEntry,
	CleanDeviceProvider,
	MockCompromisedProvider
} from './device-integrity.js';

describe('device integrity', () => {
	it('returns null warning for clean device', async () => {
		const provider = new CleanDeviceProvider();
		const warning = await checkDeviceIntegrity(provider);
		expect(warning).toBeNull();

		const audit = createIntegrityAuditEntry({ compromised: false });
		expect(audit).toBeNull();
	});

	it('returns dismissable warning for compromised device (not blocking)', async () => {
		const provider = new MockCompromisedProvider({
			compromised: true,
			reason: 'Root detected: su binary found'
		});

		const warning = await checkDeviceIntegrity(provider);
		expect(warning).not.toBeNull();
		expect(warning!.message).toContain('modified');
		expect(warning!.dismissable).toBe(true); // Patient autonomy — Nadia agreed

		const audit = createIntegrityAuditEntry({
			compromised: true,
			reason: 'Root detected: su binary found'
		});
		expect(audit).not.toBeNull();
		expect(audit!.event).toBe('compromised_device');
		expect(audit!.details).toContain('su binary');
	});
});
