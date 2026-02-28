/** BTL-10 UX: Shared elapsed time formatting — no external library. */

/**
 * Format a duration in seconds to a human-readable string.
 * - Under 60s: "45s"
 * - Under 1h: "2m 30s"
 * - Over 1h: "1h 5m"
 */
export function formatElapsed(totalSeconds: number): string {
  if (totalSeconds < 0) totalSeconds = 0;

  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m ${seconds}s`;
  return `${seconds}s`;
}

/**
 * Compute elapsed seconds between two ISO timestamps.
 * Returns 0 if either is null/undefined or if result would be negative.
 */
export function elapsedSecondsBetween(
  start: string | null | undefined,
  end: string | null | undefined,
): number {
  if (!start || !end) return 0;
  const diff = new Date(end).getTime() - new Date(start).getTime();
  return Math.max(0, Math.floor(diff / 1000));
}
