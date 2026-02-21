/**
 * I18N-22: svelte-i18n setup with synchronous locale loading.
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
 *
 * NOTE: addMessages() is synchronous — $t() works immediately after init().
 * Lazy register() + dynamic import() caused race conditions where components
 * called $t() before the locale JSON had loaded, crashing formatMessage().
 */

import { addMessages, init, getLocaleFromNavigator } from 'svelte-i18n';
import en from './locales/_generated/en.json';
import fr from './locales/_generated/fr.json';
import de from './locales/_generated/de.json';

// Load all locale data synchronously (SE-002: domain modules merged at build time)
addMessages('en', en);
addMessages('fr', fr);
addMessages('de', de);

export const SUPPORTED_LANGUAGES = new Set(['en', 'fr', 'de']);

/**
 * XD1-01: Missing key handler.
 * When $t() cannot resolve a key in any locale (including EN fallback):
 *   - Returns "[key.name]" — visually distinct, never mistaken for real text
 *   - Logs warning in dev for developer awareness
 * This prevents raw developer identifiers from rendering to users.
 */
function handleMissingMessage({ locale, id }: { locale: string; id: string }): string {
	if (import.meta.env.DEV) {
		console.warn(`[i18n] Missing key: "${id}" (locale: ${locale})`);
	}
	return `[${id}]`;
}

// I18N-04: Initialize at module level so $t() works before any component renders.
// User-preferred locale can be applied later via `locale.set()`.
const detected = getLocaleFromNavigator()?.split('-')[0] ?? 'en';
init({
	fallbackLocale: 'en',
	initialLocale: SUPPORTED_LANGUAGES.has(detected) ? detected : 'en',
	handleMissingMessage,
});
