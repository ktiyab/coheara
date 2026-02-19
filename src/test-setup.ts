/**
 * Vitest test setup file.
 * Initializes DOM matchers and svelte-i18n for component tests.
 */

import '@testing-library/jest-dom/vitest';
import { init, register } from 'svelte-i18n';

// Register English locale with inline messages for testing.
// Components use $t() keys — we provide a minimal locale so they render real text.
register('en', () =>
  import('./lib/i18n/locales/_generated/en.json')
);

init({
  fallbackLocale: 'en',
  initialLocale: 'en',
});
