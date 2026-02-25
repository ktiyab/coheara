/**
 * Colorful theme — per-component color assignments.
 * Maps each UI item to a palette hue. The colorfulStyle() helper
 * builds an inline style string that overrides --color-success cascade.
 */

import { PALETTE } from './colorful-palette';

// Nav items (7): home, chat, history, documents, timeline, companion, settings
export const NAV_HUES: string[] = [
  'rose', 'emerald', 'sky', 'indigo', 'violet', 'orange', 'teal',
];

// Feature cards (3): documents, chat, timeline
export const FEATURE_HUES: string[] = ['indigo', 'emerald', 'violet'];

// Settings sections (5): ai, preferences, extraction, privacy, about
export const SETTINGS_HUES: string[] = ['sky', 'violet', 'orange', 'teal', 'rose'];

// Privacy sections (4, danger zone excluded): data, security, sharing, backup
export const PRIVACY_HUES: string[] = ['sky', 'indigo', 'fuchsia', 'amber'];

/** Build inline style string that overrides --color-success cascade. */
export function colorfulStyle(hueName: string): string {
  const hue = PALETTE[hueName];
  if (!hue) return '';
  return `--color-success: ${hue.base}; --color-success-50: ${hue.light}; --color-success-200: ${hue.muted}; --color-success-800: ${hue.deep}`;
}
