// M1-01: Accessibility tests — 4 tests
import { describe, it, expect } from 'vitest';
import {
	computeAccessibilityConfig,
	getTouchTargetSize,
	getMaxListItems,
	getBodyTextSize,
	getHeaderTextSize,
	checkContrastRatio,
	SIMPLIFIED_LAYOUT_THRESHOLD,
	MIN_TOUCH_TARGET_DP,
	LARGE_TOUCH_TARGET_DP,
	MAMADOU_TOUCH_TARGET_DP,
	MIN_BODY_TEXT_SP,
	MIN_HEADER_TEXT_SP,
	WCAG_AAA_CONTRAST_RATIO
} from './accessibility.js';

describe('accessibility', () => {
	it('adapts layout when system font scale >= 150% (Mamadou)', () => {
		// Normal font scale
		const normalConfig = computeAccessibilityConfig(1.0, false);
		expect(normalConfig.simplifiedLayout).toBe(false);
		expect(normalConfig.fontScale).toBe(1.0);

		// At threshold
		const thresholdConfig = computeAccessibilityConfig(1.5, false);
		expect(thresholdConfig.simplifiedLayout).toBe(true);

		// Above threshold
		const largeConfig = computeAccessibilityConfig(2.0, true);
		expect(largeConfig.simplifiedLayout).toBe(true);
		expect(largeConfig.reduceMotion).toBe(true);
		expect(largeConfig.fontScale).toBe(2.0);
	});

	it('provides correct touch targets per accessibility config', () => {
		const normalConfig = computeAccessibilityConfig(1.0, false);
		const mamadouConfig = computeAccessibilityConfig(1.5, false);

		// Normal: 48dp for regular, 56dp for critical
		expect(getTouchTargetSize(normalConfig, false)).toBe(MIN_TOUCH_TARGET_DP);
		expect(getTouchTargetSize(normalConfig, true)).toBe(LARGE_TOUCH_TARGET_DP);

		// Mamadou: always 64dp
		expect(getTouchTargetSize(mamadouConfig, false)).toBe(MAMADOU_TOUCH_TARGET_DP);
		expect(getTouchTargetSize(mamadouConfig, true)).toBe(MAMADOU_TOUCH_TARGET_DP);

		// List items reduced for simplified layout
		expect(getMaxListItems(normalConfig)).toBe(10);
		expect(getMaxListItems(mamadouConfig)).toBe(5);
	});

	it('respects minimum text sizes with system font scaling', () => {
		const normal = computeAccessibilityConfig(1.0, false);
		expect(getBodyTextSize(normal)).toBe(MIN_BODY_TEXT_SP); // 18sp
		expect(getHeaderTextSize(normal)).toBe(MIN_HEADER_TEXT_SP); // 22sp

		// Scaled up
		const scaled = computeAccessibilityConfig(1.5, false);
		expect(getBodyTextSize(scaled)).toBe(27); // 18 * 1.5
		expect(getHeaderTextSize(scaled)).toBe(33); // 22 * 1.5

		// Never below minimum
		const tiny = computeAccessibilityConfig(0.5, false);
		expect(getBodyTextSize(tiny)).toBe(MIN_BODY_TEXT_SP); // Clamped to 18sp
		expect(getHeaderTextSize(tiny)).toBe(MIN_HEADER_TEXT_SP); // Clamped to 22sp
	});

	it('validates WCAG AAA contrast ratios (7:1 for body text)', () => {
		// White text on dark background (high contrast)
		const highContrast = checkContrastRatio(1.0, 0.0);
		expect(highContrast.ratio).toBeGreaterThan(WCAG_AAA_CONTRAST_RATIO);
		expect(highContrast.meetsAAA).toBe(true);

		// Similar grays (low contrast)
		const lowContrast = checkContrastRatio(0.5, 0.4);
		expect(lowContrast.meetsAAA).toBe(false);

		// Our primary color (#4A6FA5) on white (#FAFAF9)
		// Approximate luminance values
		const primaryOnWhite = checkContrastRatio(0.95, 0.14);
		expect(primaryOnWhite.ratio).toBeGreaterThan(4.5); // At least AA
	});
});
