/**
 * Tauri environment detection.
 *
 * Returns true when running inside a real Tauri webview (full-stack dev or production).
 * Returns false in plain browsers (frontend-only dev mode), stub environments, and tests.
 *
 * Note: app.html provides a __TAURI_INTERNALS__ stub for browser-only mode so that
 * Tauri API calls (invoke, listen, etc.) reject gracefully instead of crashing.
 * This function detects the stub via __TAURI_STUB__ and returns false.
 */
export function isTauriEnv(): boolean {
  try {
    return (
      typeof window !== 'undefined' &&
      !!(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ &&
      !(window as unknown as Record<string, unknown>).__TAURI_STUB__
    );
  } catch {
    return false;
  }
}
