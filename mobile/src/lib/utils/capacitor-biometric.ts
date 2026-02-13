// IMP-008: CapacitorBiometricProvider — wraps @capgo/capacitor-native-biometric
import { NativeBiometric, type AvailableResult } from '@capgo/capacitor-native-biometric';
import type { BiometricCapability } from '$lib/types/index.js';
import type { BiometricProvider } from './biometric.js';

/** Maps native biometry type to our BiometricCapability type */
function mapBiometryType(result: AvailableResult): BiometricCapability['type'] {
	if (!result.isAvailable) return 'none';
	// BiometryType enum: 1 = touchId/fingerprint, 2 = faceId, 3 = iris
	switch (result.biometryType) {
		case 1: return 'fingerprint';
		case 2: return 'face';
		case 3: return 'iris';
		default: return 'fingerprint'; // fallback for unknown types
	}
}

/** Capacitor implementation using @capgo/capacitor-native-biometric */
export class CapacitorBiometricProvider implements BiometricProvider {
	async isAvailable(): Promise<BiometricCapability> {
		try {
			const result = await NativeBiometric.isAvailable();
			return {
				available: result.isAvailable,
				type: mapBiometryType(result),
			};
		} catch {
			return { available: false, type: 'none' };
		}
	}

	async verify(reason: string): Promise<boolean> {
		try {
			await NativeBiometric.verifyIdentity({
				reason,
				title: 'Coheara',
				subtitle: 'Verify your identity',
			});
			return true;
		} catch {
			return false;
		}
	}
}
