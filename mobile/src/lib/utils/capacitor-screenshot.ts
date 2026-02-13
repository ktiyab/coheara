// IMP-012: CapacitorScreenshotProvider — wraps @capacitor/privacy-screen
import { PrivacyScreen } from '@capacitor/privacy-screen';
import type { ScreenshotProvider } from './screenshot.js';

/** Capacitor implementation using @capacitor/privacy-screen (FLAG_SECURE on Android, view hiding on iOS) */
export class CapacitorScreenshotProvider implements ScreenshotProvider {
	async enable(): Promise<void> {
		await PrivacyScreen.enable();
	}

	async disable(): Promise<void> {
		await PrivacyScreen.disable();
	}
}
