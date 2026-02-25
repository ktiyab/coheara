/**
 * Colorful theme palette — 10 curated hues, each with 4 tiers
 * matching the CSS variable contract (--color-success scale).
 */

export interface HueScale {
  base: string;   // --color-success (icon bg, active border, badge)
  light: string;  // --color-success-50 (card bg tint, hover bg)
  muted: string;  // --color-success-200 (border accent)
  deep: string;   // --color-success-800 (text on light bg)
}

export const PALETTE: Record<string, HueScale> = {
  rose:    { base: '#f43f5e', light: '#fff1f2', muted: '#fecdd3', deep: '#9f1239' },
  orange:  { base: '#f97316', light: '#fff7ed', muted: '#fed7aa', deep: '#9a3412' },
  amber:   { base: '#f59e0b', light: '#fffbeb', muted: '#fde68a', deep: '#92400e' },
  emerald: { base: '#10b981', light: '#ecfdf5', muted: '#a7f3d0', deep: '#065f46' },
  teal:    { base: '#14b8a6', light: '#f0fdfa', muted: '#99f6e4', deep: '#115e59' },
  sky:     { base: '#0ea5e9', light: '#f0f9ff', muted: '#bae6fd', deep: '#075985' },
  indigo:  { base: '#6366f1', light: '#eef2ff', muted: '#c7d2fe', deep: '#3730a3' },
  violet:  { base: '#8b5cf6', light: '#f5f3ff', muted: '#ddd6fe', deep: '#5b21b6' },
  fuchsia: { base: '#d946ef', light: '#fdf4ff', muted: '#f5d0fe', deep: '#86198f' },
  pink:    { base: '#ec4899', light: '#fdf2f8', muted: '#fbcfe8', deep: '#9d174d' },
};

export const PALETTE_KEYS = Object.keys(PALETTE) as (keyof typeof PALETTE)[];
