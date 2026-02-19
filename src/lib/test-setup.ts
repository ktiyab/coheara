import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';
import { init, addMessages } from 'svelte-i18n';
import { readFileSync } from 'fs';
import { join } from 'path';

// Load real English locale for testing (validates i18n keys work)
const generatedDir = join(__dirname, 'i18n/locales/_generated');
try {
  const en = JSON.parse(readFileSync(join(generatedDir, 'en.json'), 'utf-8'));
  addMessages('en', en);
  init({ fallbackLocale: 'en', initialLocale: 'en' });
} catch {
  // Fallback: if generated files don't exist, initialize with empty locale
  init({ fallbackLocale: 'en', initialLocale: 'en' });
}

// Mock Tauri invoke API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));

// Mock Tauri webview API
vi.mock('@tauri-apps/api/webview', () => ({
  getCurrentWebview: vi.fn(() => ({
    onDragDropEvent: vi.fn(() => Promise.resolve(() => {})),
  })),
}));

// Mock Tauri event API
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));

// Mock Tauri dialog plugin
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(() => Promise.resolve(null)),
  save: vi.fn(() => Promise.resolve(null)),
  message: vi.fn(() => Promise.resolve()),
  ask: vi.fn(() => Promise.resolve(false)),
}));
