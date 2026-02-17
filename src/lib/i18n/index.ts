/**
 * I18N-22: svelte-i18n setup with lazy-loaded JSON locale files.
 *
 * Language detection chain (I18N-04):
 *   1. User preference (stored in user_preferences table)
 *   2. System locale (navigator.language)
 *   3. Fallback to English
 *
 * Supported languages: en, fr, de (I18N-01)
 */

import { register, init, getLocaleFromNavigator } from 'svelte-i18n';

// Lazy-load locale files (only loaded when needed)
register('en', () => import('./locales/en.json'));
register('fr', () => import('./locales/fr.json'));
register('de', () => import('./locales/de.json'));

const SUPPORTED_LANGUAGES = new Set(['en', 'fr', 'de']);

/**
 * Initialize svelte-i18n with the given language preference.
 * Call this once on app startup after reading user preference from backend.
 */
export function initI18n(preferredLanguage?: string | null): void {
	// I18N-04: Detection chain
	const detected = getLocaleFromNavigator()?.split('-')[0] ?? 'en';
	const lang = preferredLanguage && SUPPORTED_LANGUAGES.has(preferredLanguage)
		? preferredLanguage
		: SUPPORTED_LANGUAGES.has(detected)
			? detected
			: 'en';

	init({
		fallbackLocale: 'en',
		initialLocale: lang,
	});
}
