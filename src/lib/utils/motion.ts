/**
 * Motion utilities for respecting system reduced-motion preferences.
 *
 * CSS-level `prefers-reduced-motion` handles most cases via app.css,
 * but Svelte `transition:` directives need JS-level duration adaptation.
 *
 * Usage:
 *   import { motionDuration } from '$lib/utils/motion';
 *   transition:fade={{ duration: motionDuration(250) }}
 */

let cachedPreference: boolean | null = null;
let mediaQuery: MediaQueryList | null = null;

function getMediaQuery(): MediaQueryList | null {
  if (typeof window === 'undefined') return null;
  if (!mediaQuery) {
    mediaQuery = window.matchMedia('(prefers-reduced-motion: reduce)');
    mediaQuery.addEventListener('change', () => {
      cachedPreference = mediaQuery!.matches;
    });
    cachedPreference = mediaQuery.matches;
  }
  return mediaQuery;
}

/** Returns true if the user prefers reduced motion. */
export function prefersReducedMotion(): boolean {
  const mq = getMediaQuery();
  if (!mq) return false;
  if (cachedPreference === null) cachedPreference = mq.matches;
  return cachedPreference;
}

/**
 * Returns the given duration if motion is allowed, or 0 if reduced motion is preferred.
 * Use with Svelte transition directives:
 *   transition:fade={{ duration: motionDuration(250) }}
 */
export function motionDuration(ms: number): number {
  return prefersReducedMotion() ? 0 : ms;
}
