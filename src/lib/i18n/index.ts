/**
 * I18N-22: svelte-i18n setup with lazy-loaded JSON locale files.
 *
 * Language detection chain (I18N-04):
 *   1. User preference (stored in user_preferences table)
 *   2. System locale (navigator.language)
 *   3. Fallback to English
 *
 * Supported languages: en, fr, de (I18N-01)
 *
 * SE-002: Locale files are built from domain modules (locales/modules/{lang}/*.json)
 * via build-locales.js → locales/_generated/{lang}.json.
 * Source of truth: locales/modules/. Edit modules, not _generated files.
 */

import { register, init, getLocaleFromNavigator } from 'svelte-i18n';

// Lazy-load locale files from build output (SE-002: domain modules merged at build time)
register('en', () => import('./locales/_generated/en.json'));
register('fr', () => import('./locales/_generated/fr.json'));
register('de', () => import('./locales/_generated/de.json'));

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
