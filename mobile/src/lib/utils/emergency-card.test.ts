// M1-01: Emergency card tests — 3 tests
import { describe, it, expect } from 'vitest';
import {
	DEFAULT_EMERGENCY_CONFIG,
	buildEmergencyCardData,
	validateEmergencyConfig,
	serializeForWidget
} from './emergency-card.js';
import type { EmergencyCardConfig } from '$lib/types/index.js';

const MOCK_PROFILE = {
	name: 'Mamadou D.',
	bloodType: 'A+',
	allergies: ['Penicillin', 'Sulfa'],
	medications: [
		{ id: 'med-1', name: 'Metformin' },
		{ id: 'med-2', name: 'Lisinopril' },
		{ id: 'med-3', name: 'Warfarin' }
	]
};

describe('emergency card', () => {
	it('returns null when card is disabled', () => {
		const data = buildEmergencyCardData(DEFAULT_EMERGENCY_CONFIG, MOCK_PROFILE);
		expect(data).toBeNull();
	});

	it('builds card data with only opted-in fields', () => {
		const config: EmergencyCardConfig = {
			enabled: true,
			showName: true,
			showBloodType: true,
			showAllergies: true,
			showEmergencyMeds: true,
			selectedMedicationIds: ['med-1', 'med-3'] // Metformin + Warfarin
		};

		const data = buildEmergencyCardData(config, MOCK_PROFILE);
		expect(data).not.toBeNull();
		expect(data!.name).toBe('Mamadou D.');
		expect(data!.bloodType).toBe('A+');
		expect(data!.allergies).toEqual(['Penicillin', 'Sulfa']);
		expect(data!.emergencyMedications).toEqual(['Metformin', 'Warfarin']);
		// Lisinopril excluded — only selected meds shown

		// Serialize for widget
		const text = serializeForWidget(data!);
		expect(text).toContain('EMERGENCY HEALTH INFO');
		expect(text).toContain('Mamadou D.');
		expect(text).toContain('Blood type: A+');
		expect(text).toContain('Allergies: Penicillin, Sulfa');
		expect(text).toContain('Key meds: Metformin, Warfarin');
		expect(text).toContain('Coheara');
	});

	it('respects data isolation — omitted fields do not appear', () => {
		const config: EmergencyCardConfig = {
			enabled: true,
			showName: false,
			showBloodType: false,
			showAllergies: true,
			showEmergencyMeds: false,
			selectedMedicationIds: []
		};

		const data = buildEmergencyCardData(config, MOCK_PROFILE);
		expect(data).not.toBeNull();
		expect(data!.name).toBe(''); // Not shown
		expect(data!.bloodType).toBeUndefined(); // Not shown
		expect(data!.allergies).toEqual(['Penicillin', 'Sulfa']); // Shown
		expect(data!.emergencyMedications).toEqual([]); // Not shown

		// Validate config warns on too many meds
		const valid = validateEmergencyConfig(config);
		expect(valid.valid).toBe(true);
		expect(valid.warnings).toHaveLength(0);

		// Too many selected meds
		const bigConfig = { ...config, selectedMedicationIds: Array(15).fill('id') };
		const bigValid = validateEmergencyConfig(bigConfig);
		expect(bigValid.warnings.length).toBeGreaterThan(0);
	});
});
