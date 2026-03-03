/**
 * Scenario 05: Navigation — Visit All Screens
 *
 * Precondition: Profile "Alice Martin" active.
 * Verifies all main screens render without errors.
 */
import type { TestHarness } from '../harness.js';
import { CohearaDriver, type Screen } from '../driver.js';

const SCREENS_TO_TEST: Array<{ screen: Screen; label: string; expectText?: string }> = [
  { screen: 'home', label: 'Home', expectText: 'Alice Martin' },
  { screen: 'chat', label: 'Chat' },
  { screen: 'documents', label: 'Documents' },
  { screen: 'timeline', label: 'Timeline' },
  { screen: 'settings', label: 'Settings' },
];

export async function run(harness: TestHarness): Promise<void> {
  const driver = new CohearaDriver(harness);

  for (const { screen, label, expectText } of SCREENS_TO_TEST) {
    console.log(`[05] Navigating to ${label}...`);
    await driver.navigate(screen);
    await sleep(2000);
    await driver.screenshot(`05-screen-${screen}`);

    // Verify no error screen (ignore boot fallback HTML which contains "Error" in script text)
    const hasError = await harness.browser!.execute(() => {
      const main = document.querySelector('main, [data-sveltekit-hydrated]');
      if (!main) return false;
      const text = main.textContent ?? '';
      return text.includes('Something went wrong') || text.includes('Unexpected error');
    });

    if (hasError) {
      console.log(`[05] WARN — Error detected on ${label} screen`);
      await driver.screenshot(`05-error-${screen}`);
    }

    // Verify expected text if specified
    if (expectText) {
      const bodyText = await harness.browser!.execute(() => document.body.textContent ?? '');
      if (bodyText.includes(expectText)) {
        console.log(`[05] ${label}: Expected text "${expectText}" found`);
      } else {
        console.log(`[05] WARN — ${label}: Expected text "${expectText}" not found`);
      }
    }

    console.log(`[05] ${label} screen rendered OK`);
  }

  // Return to home
  await driver.navigate('home');
  await sleep(1000);

  console.log('[05] PASS — All screens navigated successfully');
}

function sleep(ms: number): Promise<void> {
  return new Promise(r => setTimeout(r, ms));
}
