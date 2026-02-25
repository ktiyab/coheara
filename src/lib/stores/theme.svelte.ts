/**
 * Theme Store — D2 Design System (Spec 63)
 *
 * Manages 3 themes: light (default), dark, colorful.
 * Theme applied via CSS class on <html>: .dark or .colorful.
 * Persisted in encrypted profile via IPC.
 * Falls back to OS preference on first launch.
 */
import { isTauriEnv } from '$lib/utils/tauri';

export type Theme = 'light' | 'dark' | 'colorful';

const VALID_THEMES: ReadonlySet<Theme> = new Set(['light', 'dark', 'colorful']);

class ThemeStore {
  current: Theme = $state('light');

  /** Apply theme to DOM immediately (no persistence). */
  apply(theme: Theme) {
    if (!VALID_THEMES.has(theme)) return;
    this.current = theme;

    if (typeof document === 'undefined') return;
    const html = document.documentElement;
    html.classList.remove('dark', 'colorful');
    if (theme === 'dark') html.classList.add('dark');
    if (theme === 'colorful') html.classList.add('colorful');
  }

  /** Initialize from persisted preference or OS default. */
  async init() {
    if (isTauriEnv()) {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const pref = await invoke<string | null>('get_user_preference_cmd', { key: 'theme' });
        if (pref && VALID_THEMES.has(pref as Theme)) {
          this.apply(pref as Theme);
          return;
        }
      } catch {
        // IPC unavailable — fall through to OS detection
      }
    }

    // Fall back to OS preference
    if (typeof window !== 'undefined' && window.matchMedia('(prefers-color-scheme: dark)').matches) {
      this.apply('dark');
    } else {
      this.apply('light');
    }
  }

  get isColorful(): boolean {
    return this.current === 'colorful';
  }

  /** Set theme, apply to DOM, and persist via IPC. */
  async set(newTheme: Theme) {
    this.apply(newTheme);

    if (isTauriEnv()) {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('set_user_preference_cmd', { key: 'theme', value: newTheme });
      } catch {
        // Persistence failed — theme still applied to DOM
      }
    }
  }
}

export const theme = new ThemeStore();
