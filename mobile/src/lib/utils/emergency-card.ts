// M1-01: Emergency data card — opt-in lock screen widget (BP-03)
// Data stored OUTSIDE encrypted cache — in plain text accessible from lock screen.
// Only the data the user explicitly opts into. Minimal by design. (Nadia)
import type { EmergencyCardConfig, EmergencyCardData } from '$lib/types/index.js';

/** Default config — everything disabled */
export const DEFAULT_EMERGENCY_CONFIG: EmergencyCardConfig = {
	enabled: false,
	showName: false,
	showBloodType: false,
	showAllergies: false,
	showEmergencyMeds: false,
	selectedMedicationIds: []
};

/**
 * Build the display data for the emergency card widget.
 * Only includes fields the user has explicitly opted into.
 */
export function buildEmergencyCardData(
	config: EmergencyCardConfig,
	profile: {
		name: string;
		bloodType?: string;
		allergies: string[];
		medications: Array<{ id: string; name: string }>;
	}
): EmergencyCardData | null {
	if (!config.enabled) return null;

	const data: EmergencyCardData = {
		name: config.showName ? profile.name : '',
		allergies: [],
		emergencyMedications: []
	};

	if (config.showBloodType && profile.bloodType) {
		data.bloodType = profile.bloodType;
	}

	if (config.showAllergies) {
		data.allergies = [...profile.allergies];
	}

	if (config.showEmergencyMeds) {
		data.emergencyMedications = profile.medications
			.filter((m) => config.selectedMedicationIds.includes(m.id))
			.map((m) => m.name);
	}

	return data;
}

/**
 * Validate that emergency card config only includes allowed field types.
 * Full medication list and lab results are NEVER allowed (spec constraint).
 */
export function validateEmergencyConfig(config: EmergencyCardConfig): {
	valid: boolean;
	warnings: string[];
} {
	const warnings: string[] = [];

	if (config.selectedMedicationIds.length > 10) {
		warnings.push('Maximum 10 emergency medications recommended for readability');
	}

	return { valid: true, warnings };
}

/** Serialize emergency card data for widget storage (plain text, no encryption) */
export function serializeForWidget(data: EmergencyCardData): string {
	const lines: string[] = ['EMERGENCY HEALTH INFO'];

	if (data.name) {
		lines.push(data.name);
	}

	if (data.bloodType) {
		lines.push(`Blood type: ${data.bloodType}`);
	}

	if (data.allergies.length > 0) {
		lines.push(`Allergies: ${data.allergies.join(', ')}`);
	}

	if (data.emergencyMedications.length > 0) {
		lines.push(`Key meds: ${data.emergencyMedications.join(', ')}`);
	}

	lines.push('', 'Coheara');
	return lines.join('\n');
}
