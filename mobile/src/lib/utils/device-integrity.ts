// M1-01: Device integrity — root/jailbreak detection (warning, not blocking)
import type { DeviceIntegrityResult } from '$lib/types/index.js';

/** Provider interface for native device integrity check */
export interface DeviceIntegrityProvider {
	check(): Promise<DeviceIntegrityResult>;
}

/** Default provider for clean devices */
export class CleanDeviceProvider implements DeviceIntegrityProvider {
	async check(): Promise<DeviceIntegrityResult> {
		return { compromised: false };
	}
}

/** Mock provider for testing compromised device scenarios */
export class MockCompromisedProvider implements DeviceIntegrityProvider {
	private result: DeviceIntegrityResult;

	constructor(result: DeviceIntegrityResult) {
		this.result = result;
	}

	async check(): Promise<DeviceIntegrityResult> {
		return this.result;
	}
}

/** Warning state for compromised devices */
export interface IntegrityWarning {
	shown: boolean;
	message: string;
	dismissable: boolean;
}

/**
 * Check device integrity and generate a warning if compromised.
 * Per Nadia's requirement: warning only, not blocking (patient autonomy).
 */
export async function checkDeviceIntegrity(
	provider: DeviceIntegrityProvider
): Promise<IntegrityWarning | null> {
	const result = await provider.check();

	if (!result.compromised) {
		return null;
	}

	return {
		shown: false,
		message: 'Your device may be modified. Health data on this phone may be less secure.',
		dismissable: true
	};
}

/** Create an audit entry for compromised device (queued for next desktop sync) */
export function createIntegrityAuditEntry(result: DeviceIntegrityResult): {
	event: string;
	details: string;
} | null {
	if (!result.compromised) return null;

	return {
		event: 'compromised_device',
		details: result.reason ?? 'unknown'
	};
}
