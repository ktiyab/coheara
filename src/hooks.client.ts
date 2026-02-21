/**
 * SvelteKit client error hook.
 * Logs unhandled errors with full context — no more silent failures.
 */
import type { HandleClientError } from '@sveltejs/kit';

export const handleError: HandleClientError = ({ error, event }) => {
  const err = error instanceof Error ? error : new Error(String(error));

  console.error(
    `[Coheara] Unhandled client error during ${event.route.id ?? 'unknown route'}:`,
    err.message,
    '\n',
    err.stack ?? '(no stack trace)',
  );

  return {
    message: err.message,
  };
};
