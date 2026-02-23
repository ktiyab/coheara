// M1-01: Screenshot prevention — sensitive screens blocked, share as alternative (BP-02)
import type { SensitiveScreen } from '$lib/types/index.js';

/** Provider interface for native screenshot prevention */
export interface ScreenshotProvider {
	enable(): Promise<void>;
	disable(): Promise<void>;
}

/** No-op provider for non-native environments */
export class NoOpScreenshotProvider implements ScreenshotProvider {
	async enable(): Promise<void> { /* no-op */ }
	async disable(): Promise<void> { /* no-op */ }
}

/** Mock provider for testing */
export class MockScreenshotProvider implements ScreenshotProvider {
	public enabled = false;

	async enable(): Promise<void> {
		this.enabled = true;
	}

	async disable(): Promise<void> {
		this.enabled = false;
	}
}

/** Screens that require screenshot prevention (Nadia BP-02) */
export const SENSITIVE_SCREENS: ReadonlySet<SensitiveScreen> = new Set([
	'medications',
	'labs',
	'alerts',
	'ask',
	'journal',
	'appointment_prep'
]);

/** Non-sensitive screens (screenshot allowed) */
const NON_SENSITIVE = new Set(['home', 'settings', 'pairing', 'documents']);

/** Check if a screen requires screenshot prevention */
export function isSensitiveScreen(screenId: string): boolean {
	return SENSITIVE_SCREENS.has(screenId as SensitiveScreen);
}

/**
 * Manage screenshot prevention based on current screen.
 * Every screen with prevention has a [Share] button as alternative.
 */
export async function updateScreenshotPrevention(
	provider: ScreenshotProvider,
	screenId: string
): Promise<void> {
	if (isSensitiveScreen(screenId)) {
		await provider.enable();
	} else {
		await provider.disable();
	}
}

/** Generate a plain-text share summary for a sensitive screen */
export function generateShareText(screenId: SensitiveScreen, data: Record<string, string>): string {
	const lines: string[] = [`Coheara — ${screenId}`];

	for (const [key, value] of Object.entries(data)) {
		lines.push(`${key}: ${value}`);
	}

	lines.push('', 'Shared from Coheara');
	return lines.join('\n');
}
