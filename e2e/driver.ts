/**
 * E2E-01 Brick 3: CohearaDriver — Domain-specific WebDriver client.
 *
 * Wraps WebdriverIO with Coheara-specific navigation, form interaction,
 * data seeding, screenshots, and assertions.
 */

import type { Browser, Element } from 'webdriverio';
import type { TestHarness } from './harness.js';

// ── Types ────────────────────────────────────────────────────────────────────

export type Screen =
  | 'home' | 'chat' | 'history' | 'documents' | 'document-detail'
  | 'review' | 'timeline' | 'settings' | 'ai-settings' | 'privacy'
  | 'companion' | 'profiles' | 'profiles-create';

export interface FormField {
  selector: string;
  value: string;
  type: 'text' | 'password' | 'radio' | 'checkbox' | 'select';
}

// ── Screen Selectors ─────────────────────────────────────────────────────────

/**
 * Identify screens by unique DOM elements that only exist on that screen.
 * Falls back to hash-based detection if elements not found.
 */
const SCREEN_SELECTORS: Partial<Record<Screen, string>> = {
  home: '[data-screen="home"], .home-screen, h2',
  chat: '[data-screen="chat"], textarea[placeholder], .chat-input',
  history: '[data-screen="history"]',
  documents: '[data-screen="documents"]',
  timeline: '[data-screen="timeline"]',
  settings: '[data-screen="settings"]',
  profiles: '[data-screen="profiles"]',
  companion: '[data-screen="companion"]',
};

// ── CohearaDriver ────────────────────────────────────────────────────────────

export class CohearaDriver {
  private browser: Browser;
  private harness: TestHarness;

  constructor(harness: TestHarness) {
    if (!harness.browser) throw new Error('Harness browser not connected');
    this.browser = harness.browser;
    this.harness = harness;
  }

  // ── Navigation ─────────────────────────────────────────────────────────

  async navigate(screen: Screen, params?: Record<string, string>): Promise<void> {
    // Use hash change which triggers NavigationStore.readFromHash() via hashchange listener.
    // Also try direct store navigate() if the global store is exposed.
    await this.browser.execute((s: string, p: Record<string, string> | undefined) => {
      // Try setting hash (works for SvelteKit hash-based routing)
      const hash = p ? s + '?' + new URLSearchParams(p).toString() : s;
      window.location.hash = hash;
      // Force a hashchange event in case the browser optimizes it away
      window.dispatchEvent(new HashChangeEvent('hashchange'));
    }, screen as string, params);

    // Wait for navigation to settle
    await this.sleep(1000);
  }

  async waitForScreen(screen: Screen, timeoutMs = 10_000): Promise<void> {
    const selector = SCREEN_SELECTORS[screen];
    if (selector) {
      try {
        const el = await this.browser.$(selector);
        await el.waitForDisplayed({ timeout: timeoutMs });
        return;
      } catch { /* fall through to hash-based check */ }
    }

    // Hash-based fallback
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      const hash = await this.browser.execute(() => window.location.hash.slice(1).split('?')[0]);
      if (hash === screen) return;
      await this.sleep(300);
    }

    // Timeout — screenshot for diagnostics
    await this.harness.screenshot(`timeout-${screen}`);
    throw new Error(`Screen '${screen}' did not appear within ${timeoutMs}ms`);
  }

  async getCurrentScreen(): Promise<string> {
    return this.browser.execute(() => {
      const hash = window.location.hash.slice(1);
      return hash.split('?')[0] || 'home';
    });
  }

  // ── Form Interaction ───────────────────────────────────────────────────

  async type(selector: string, text: string): Promise<void> {
    const el = await this.browser.$(selector);
    await el.waitForDisplayed({ timeout: 5000 });
    await el.clearValue();
    await el.setValue(text);
    // Dispatch input event to trigger Svelte reactivity
    await this.browser.execute((sel: string) => {
      const el = document.querySelector(sel);
      if (el) {
        el.dispatchEvent(new Event('input', { bubbles: true }));
        el.dispatchEvent(new Event('change', { bubbles: true }));
      }
    }, selector);
  }

  async click(selector: string): Promise<void> {
    const el = await this.browser.$(selector);
    await el.waitForDisplayed({ timeout: 5000 });
    await el.click();
  }

  async clickByText(text: string): Promise<void> {
    const el = await this.browser.$(`//*[contains(text(), "${text}")]`);
    await el.waitForDisplayed({ timeout: 5000 });
    await el.click();
  }

  async selectRadio(name: string, value: string): Promise<void> {
    // Click the label containing the radio input
    const selector = `input[type="radio"][name="${name}"][value="${value}"]`;
    const radio = await this.browser.$(selector);
    const parent = await radio.parentElement();
    await parent.click();
  }

  async toggleCheckbox(selector: string): Promise<void> {
    const el = await this.browser.$(selector);
    await el.click();
  }

  async selectDropdown(selector: string, value: string): Promise<void> {
    const el = await this.browser.$(selector);
    await el.selectByAttribute('value', value);
  }

  async fillForm(fields: FormField[]): Promise<void> {
    for (const field of fields) {
      switch (field.type) {
        case 'text':
        case 'password':
          await this.type(field.selector, field.value);
          break;
        case 'radio':
          await this.selectRadio(field.selector, field.value);
          break;
        case 'checkbox':
          await this.toggleCheckbox(field.selector);
          break;
        case 'select':
          await this.selectDropdown(field.selector, field.value);
          break;
      }
    }
  }

  // ── Data Access ────────────────────────────────────────────────────────

  /**
   * Execute a Tauri IPC command directly in the webview.
   * Bypasses UI for data operations (import documents, seed data).
   */
  async executeInvoke<T = unknown>(command: string, args?: Record<string, unknown>): Promise<T> {
    // JSON-stringify the result inside the webview to avoid WebDriver serialization issues.
    // WebKitGTK's WebDriver can't serialize all Rust types (Vec<String>, complex enums, etc.)
    const json = await this.browser.execute(
      (cmd: string, a: Record<string, unknown> | undefined) => {
        const tauri = (window as any).__TAURI_INTERNALS__;
        if (!tauri?.invoke) throw new Error('Tauri IPC not available');
        return tauri.invoke(cmd, a ?? {}).then(
          (r: unknown) => JSON.stringify({ ok: true, data: r }),
          (e: unknown) => JSON.stringify({ ok: false, error: String(e) }),
        );
      },
      command,
      args,
    ) as string;
    const parsed = JSON.parse(json);
    if (!parsed.ok) throw new Error(parsed.error);
    return parsed.data as T;
  }

  async getText(selector: string): Promise<string> {
    const el = await this.browser.$(selector);
    await el.waitForDisplayed({ timeout: 5000 });
    return el.getText();
  }

  async getInputValue(selector: string): Promise<string> {
    const el = await this.browser.$(selector);
    return el.getValue();
  }

  async isVisible(selector: string): Promise<boolean> {
    try {
      const el = await this.browser.$(selector);
      return await el.isDisplayed();
    } catch {
      return false;
    }
  }

  async waitForText(text: string, timeoutMs = 30_000): Promise<void> {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      const bodyText = await this.browser.execute(() => document.body.textContent ?? '');
      if (bodyText.includes(text)) return;
      await this.sleep(500);
    }
    await this.harness.screenshot(`timeout-text-${text.slice(0, 20).replace(/\s/g, '_')}`);
    throw new Error(`Text "${text}" did not appear within ${timeoutMs}ms`);
  }

  async waitForElement(selector: string, timeoutMs = 10_000): Promise<Element> {
    const el = await this.browser.$(selector);
    await el.waitForExist({ timeout: timeoutMs });
    return el;
  }

  // ── Screenshots ────────────────────────────────────────────────────────

  async screenshot(name: string): Promise<string> {
    return this.harness.screenshot(name);
  }

  // ── Assertions ─────────────────────────────────────────────────────────

  async assertVisible(selector: string, message?: string): Promise<void> {
    const visible = await this.isVisible(selector);
    if (!visible) {
      await this.harness.screenshot(`assert-fail-visible`);
      throw new Error(message ?? `Expected element "${selector}" to be visible`);
    }
  }

  async assertNotVisible(selector: string, message?: string): Promise<void> {
    const visible = await this.isVisible(selector);
    if (visible) {
      await this.harness.screenshot(`assert-fail-not-visible`);
      throw new Error(message ?? `Expected element "${selector}" to NOT be visible`);
    }
  }

  async assertText(selector: string, expected: string): Promise<void> {
    const actual = await this.getText(selector);
    if (!actual.includes(expected)) {
      await this.harness.screenshot(`assert-fail-text`);
      throw new Error(`Expected text "${expected}" in "${selector}", got "${actual}"`);
    }
  }

  async assertScreenIs(screen: Screen): Promise<void> {
    const current = await this.getCurrentScreen();
    if (current !== screen) {
      await this.harness.screenshot(`assert-fail-screen`);
      throw new Error(`Expected screen "${screen}", got "${current}"`);
    }
  }

  async assertBodyContains(text: string): Promise<void> {
    const bodyText = await this.browser.execute(() => document.body.textContent ?? '');
    if (!bodyText.includes(text)) {
      await this.harness.screenshot(`assert-fail-body`);
      throw new Error(`Expected body to contain "${text}"`);
    }
  }

  // ── Utility ────────────────────────────────────────────────────────────

  /** Get the raw WebdriverIO browser for advanced operations. */
  get raw(): Browser {
    return this.browser;
  }

  private sleep(ms: number): Promise<void> {
    return new Promise(r => setTimeout(r, ms));
  }
}
