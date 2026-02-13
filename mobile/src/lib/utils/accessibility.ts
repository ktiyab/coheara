// M1-01: Accessibility logic — Mamadou-driven requirements (WCAG AAA)
import type { AccessibilityConfig } from '$lib/types/index.js';

/** Default accessibility configuration */
export const DEFAULT_ACCESSIBILITY: AccessibilityConfig = {
	fontScale: 1.0,
	reduceMotion: false,
	highContrast: false,
	simplifiedLayout: false
};

/** Threshold for simplified layout (Mamadou spec: >= 150%) */
export const SIMPLIFIED_LAYOUT_THRESHOLD = 1.5;

/** Minimum touch target size in dp */
export const MIN_TOUCH_TARGET_DP = 48;

/** Large touch target size for Mamadou-critical buttons */
export const LARGE_TOUCH_TARGET_DP = 56;

/** Mamadou large font touch target */
export const MAMADOU_TOUCH_TARGET_DP = 64;

/** Minimum body text size in sp */
export const MIN_BODY_TEXT_SP = 18;

/** Minimum header text size in sp */
export const MIN_HEADER_TEXT_SP = 22;

/** WCAG AAA contrast ratio for body text */
export const WCAG_AAA_CONTRAST_RATIO = 7.0;

/**
 * Compute accessibility config from system settings.
 *
 * When system font scale >= 150%:
 * - Simplify home screen: medication list only
 * - Increase button height to 64dp
 * - Reduce information density
 * - Show fewer items per list (5 instead of 10)
 */
export function computeAccessibilityConfig(
	systemFontScale: number,
	systemReduceMotion: boolean
): AccessibilityConfig {
	const simplified = systemFontScale >= SIMPLIFIED_LAYOUT_THRESHOLD;

	return {
		fontScale: systemFontScale,
		reduceMotion: systemReduceMotion,
		highContrast: false, // Set from system preference at runtime
		simplifiedLayout: simplified
	};
}

/** Get the appropriate touch target size based on accessibility config */
export function getTouchTargetSize(config: AccessibilityConfig, critical: boolean): number {
	if (config.simplifiedLayout) {
		return MAMADOU_TOUCH_TARGET_DP;
	}
	return critical ? LARGE_TOUCH_TARGET_DP : MIN_TOUCH_TARGET_DP;
}

/** Get max list items based on accessibility config */
export function getMaxListItems(config: AccessibilityConfig): number {
	return config.simplifiedLayout ? 5 : 10;
}

/** Get body text size respecting system font scale */
export function getBodyTextSize(config: AccessibilityConfig): number {
	return Math.max(MIN_BODY_TEXT_SP, MIN_BODY_TEXT_SP * config.fontScale);
}

/** Get header text size respecting system font scale */
export function getHeaderTextSize(config: AccessibilityConfig): number {
	return Math.max(MIN_HEADER_TEXT_SP, MIN_HEADER_TEXT_SP * config.fontScale);
}

/**
 * Validate that a color pair meets WCAG AAA contrast requirements.
 * Uses relative luminance formula from WCAG 2.1.
 */
export function checkContrastRatio(
	foregroundLuminance: number,
	backgroundLuminance: number
): { ratio: number; meetsAAA: boolean } {
	const lighter = Math.max(foregroundLuminance, backgroundLuminance);
	const darker = Math.min(foregroundLuminance, backgroundLuminance);
	const ratio = (lighter + 0.05) / (darker + 0.05);

	return {
		ratio,
		meetsAAA: ratio >= WCAG_AAA_CONTRAST_RATIO
	};
}
