// M1-01: Screenshot prevention tests — 2 tests
import { describe, it, expect } from 'vitest';
import {
	isSensitiveScreen,
	updateScreenshotPrevention,
	generateShareText,
	MockScreenshotProvider,
	SENSITIVE_SCREENS
} from './screenshot.js';

describe('screenshot prevention', () => {
	it('flags sensitive screens and allows non-sensitive ones', async () => {
		// Sensitive screens (Nadia BP-02)
		expect(isSensitiveScreen('medications')).toBe(true);
		expect(isSensitiveScreen('labs')).toBe(true);
		expect(isSensitiveScreen('alerts')).toBe(true);
		expect(isSensitiveScreen('chat')).toBe(true);
		expect(isSensitiveScreen('journal')).toBe(true);
		expect(isSensitiveScreen('appointment_prep')).toBe(true);

		// Non-sensitive screens
		expect(isSensitiveScreen('home')).toBe(false);
		expect(isSensitiveScreen('settings')).toBe(false);
		expect(isSensitiveScreen('pairing')).toBe(false);

		// Total: 6 sensitive screens
		expect(SENSITIVE_SCREENS.size).toBe(6);

		// Provider receives correct enable/disable calls
		const provider = new MockScreenshotProvider();

		await updateScreenshotPrevention(provider, 'medications');
		expect(provider.enabled).toBe(true);

		await updateScreenshotPrevention(provider, 'home');
		expect(provider.enabled).toBe(false);
	});

	it('provides text share alternative for sensitive screens', () => {
		const shareText = generateShareText('medications', {
			'Medication': 'Metformin 500mg',
			'Schedule': 'Morning and evening'
		});

		expect(shareText).toContain('Coheara');
		expect(shareText).toContain('medications');
		expect(shareText).toContain('Metformin 500mg');
		expect(shareText).toContain('Morning and evening');
	});
});
