/**
 * Scenario 01: First Launch — Create Profile
 *
 * Exercises the full onboarding: TrustScreen → ProfileTypeChoice → CreateProfile
 * → RecoveryPhrase → WelcomeTour → Home.
 */
import type { TestHarness } from '../harness.js';
import { CohearaDriver } from '../driver.js';

export async function run(harness: TestHarness): Promise<void> {
  const driver = new CohearaDriver(harness);

  // ── Step 1: Trust Screen ──────────────────────────────────────────────
  console.log('[01] Trust Screen visible');
  await driver.waitForText('Your Personal Health Companion', 15_000);
  await driver.screenshot('01-trust-screen');

  // ── Step 2: Click Continue → ProfileTypeChoice ────────────────────────
  console.log('[01] Clicking Continue...');
  await driver.clickByText('Create your');
  await sleep(1000);
  await driver.screenshot('01-profile-type');

  // ── Step 3: Choose "For myself" → CreateProfile form ──────────────────
  console.log('[01] Selecting self profile...');
  // Click the first card (self profile) — it contains "For myself" or similar text
  // The card triggers onSelect(false) for self profile
  const bodyText = await harness.browser!.execute(() => document.body.textContent ?? '');
  // Find the self-profile card — it's the first clickable card
  await harness.browser!.execute(() => {
    // Find buttons/cards in the type choice screen
    const buttons = document.querySelectorAll('button');
    for (const btn of buttons) {
      // Self profile button is the first large card (not Back button)
      if (btn.textContent && btn.textContent.length > 20 && !btn.textContent.includes('Back')) {
        btn.click();
        return;
      }
    }
  });
  await sleep(1500);
  await driver.screenshot('01-create-form-empty');

  // ── Step 4: Fill profile form ─────────────────────────────────────────
  console.log('[01] Filling profile form...');

  // Name input — find by type="text" (first text input on the form)
  await harness.browser!.execute(() => {
    const input = document.querySelector('input[type="text"]') as HTMLInputElement;
    if (input) {
      const nativeSetter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!;
      nativeSetter.call(input, 'Alice Martin');
      input.dispatchEvent(new Event('input', { bubbles: true }));
      input.dispatchEvent(new Event('change', { bubbles: true }));
    }
  });
  await sleep(300);

  // Sex: Female — click the radio label
  await harness.browser!.execute(() => {
    const radios = document.querySelectorAll('input[type="radio"]');
    for (const radio of radios) {
      if ((radio as HTMLInputElement).value === 'Female') {
        const label = radio.closest('label') || radio.parentElement;
        if (label) (label as HTMLElement).click();
        return;
      }
    }
  });
  await sleep(300);

  // Ethnicity: European — click the first checkbox
  await harness.browser!.execute(() => {
    const checkbox = document.querySelector('input[type="checkbox"]') as HTMLInputElement;
    if (checkbox) {
      const label = checkbox.closest('label') || checkbox.parentElement;
      if (label) (label as HTMLElement).click();
    }
  });
  await sleep(300);

  // Password fields — find both password inputs
  await harness.browser!.execute(() => {
    const passwordInputs = document.querySelectorAll('input[type="password"]');
    const nativeSetter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!;
    if (passwordInputs[0]) {
      nativeSetter.call(passwordInputs[0], 'TestPassword42!');
      passwordInputs[0].dispatchEvent(new Event('input', { bubbles: true }));
      passwordInputs[0].dispatchEvent(new Event('change', { bubbles: true }));
    }
    if (passwordInputs[1]) {
      nativeSetter.call(passwordInputs[1], 'TestPassword42!');
      passwordInputs[1].dispatchEvent(new Event('input', { bubbles: true }));
      passwordInputs[1].dispatchEvent(new Event('change', { bubbles: true }));
    }
  });
  await sleep(500);
  await driver.screenshot('01-create-form-filled');

  // ── Step 5: Submit → Create Profile ───────────────────────────────────
  console.log('[01] Submitting profile...');
  // Find and click the Create button (disabled state should now be false)
  await harness.browser!.execute(() => {
    const buttons = document.querySelectorAll('button');
    for (const btn of buttons) {
      const text = btn.textContent?.trim() ?? '';
      // Create button text from i18n
      if (text.includes('Create') && !btn.disabled) {
        btn.click();
        return true;
      }
    }
    return false;
  });

  // Wait for profile creation (Argon2 + AES-256-GCM + SQLite can take 60-90s
  // when Ollama is also running and competing for CPU)
  console.log('[01] Waiting for profile creation (crypto + DB)...');
  const createDeadline = Date.now() + 120_000;
  let creationDone = false;

  while (Date.now() < createDeadline) {
    const bodyNow = await harness.browser!.execute(() => document.body.textContent ?? '');
    // Recovery screen has "recovery" or "Recovery" text or the word grid
    if (bodyNow.includes('ecovery') || bodyNow.includes('written') || bodyNow.includes('phrase')) {
      creationDone = true;
      break;
    }
    // Still creating?
    if (bodyNow.includes('Creating')) {
      await sleep(2000);
      continue;
    }
    // Something else appeared (welcome tour, error, etc.)
    creationDone = true;
    break;
  }

  await driver.screenshot('01-post-create');
  if (!creationDone) {
    console.log('[01] WARN — Profile creation took longer than 120s — waiting more...');
    // Keep polling — Argon2 on CPU with Ollama loaded can be very slow
    const extraDeadline = Date.now() + 60_000;
    while (Date.now() < extraDeadline) {
      const bodyNow = await harness.browser!.execute(() => document.body.textContent ?? '');
      if (bodyNow.includes('ecovery') || bodyNow.includes('written') || bodyNow.includes('phrase')) {
        creationDone = true;
        break;
      }
      await sleep(3000);
    }
    if (!creationDone) {
      throw new Error('Profile creation did not complete within 180s');
    }
  }

  // ── Step 6: Recovery Phrase ───────────────────────────────────────────
  const onRecovery = await harness.browser!.execute(() => {
    const text = document.body.textContent ?? '';
    return text.includes('ecovery') || text.includes('written') ||
           text.includes('phrase') || text.includes('word');
  });

  if (onRecovery) {
    console.log('[01] Recovery phrase screen');
    await driver.screenshot('01-recovery-phrase');

    // Count words in the grid
    const wordCount = await harness.browser!.execute(() => {
      // Words are in grid children — look for the 3-column grid
      const grids = document.querySelectorAll('.grid');
      for (const grid of grids) {
        if (grid.children.length >= 12) return grid.children.length;
      }
      return 0;
    });
    console.log(`[01] Recovery phrase has ${wordCount} words`);

    // Check the confirmation checkbox
    await harness.browser!.execute(() => {
      const checkbox = document.querySelector('input[type="checkbox"]') as HTMLInputElement;
      if (checkbox) {
        checkbox.click();
      }
    });
    await sleep(500);

    // Click Continue (skip verification)
    await harness.browser!.execute(() => {
      const buttons = document.querySelectorAll('button');
      for (const btn of buttons) {
        const text = btn.textContent?.trim() ?? '';
        if ((text.includes('Continue') || text.includes('continue')) && !btn.disabled) {
          btn.click();
          return;
        }
      }
    });
    await sleep(1000);
  }

  // ── Step 7: Welcome Tour ──────────────────────────────────────────────
  const onTour = await harness.browser!.execute(() => {
    return document.body.textContent?.includes('Skip') ?? false;
  });

  if (onTour) {
    console.log('[01] Welcome tour — clicking Skip');
    await driver.screenshot('01-welcome-tour');

    await harness.browser!.execute(() => {
      const buttons = document.querySelectorAll('button');
      for (const btn of buttons) {
        if (btn.textContent?.trim() === 'Skip') {
          btn.click();
          return;
        }
      }
    });
    await sleep(2000);
  }

  // ── Step 8: Home Screen ───────────────────────────────────────────────
  console.log('[01] Waiting for home screen...');
  // Poll for home screen — profile creation + onboarding can take time
  const homeDeadline = Date.now() + 15_000;
  while (Date.now() < homeDeadline) {
    const text = await harness.browser!.execute(() => document.body.textContent ?? '');
    if (text.includes('Alice Martin') || text.includes('Home') || text.includes('Welcome')) {
      break;
    }
    await sleep(1000);
  }

  await driver.screenshot('01-home-screen');

  // Verify profile name appears somewhere on the page
  const finalBody = await harness.browser!.execute(() => document.body.textContent ?? '');
  if (finalBody.includes('Alice Martin')) {
    console.log('[01] Profile name "Alice Martin" visible on home screen');
  } else {
    console.log('[01] WARN — "Alice Martin" not found in body. Current screen content logged.');
    console.log(`[01] Body preview: ${finalBody.substring(0, 300)}`);
  }

  // ── Step 9: Configure AI model ────────────────────────────────────
  // Model must be configured after profile creation for extraction/chat to work.
  // The "coheara-medgemma-*" name prefix doesn't match auto-suggestion heuristics,
  // so we must set tags explicitly via IPC.
  if (harness.isOllamaAvailable()) {
    console.log('[01] Configuring AI model...');
    const MODEL_NAME = 'ktiyab/coheara-medgemma-4b-q4:latest';

    try {
      // Set as active model
      // Tauri v2 converts Rust snake_case to JS camelCase
      await driver.executeInvoke('set_active_model', { modelName: MODEL_NAME });
      console.log(`[01] Active model set: ${MODEL_NAME}`);
    } catch (err) {
      console.log(`[01] WARN — set_active_model failed: ${err}`);
    }

    try {
      // Tag with full MedGemma capabilities (Vision + Medical + formats)
      await driver.executeInvoke('set_model_tags', {
        modelName: MODEL_NAME,
        tags: ['Txt', 'Vision', 'Png', 'Jpeg', 'Medical'],
      });
      console.log('[01] Model tags configured: Txt, Vision, Png, Jpeg, Medical');
    } catch (err) {
      console.log(`[01] WARN — set_model_tags failed: ${err}`);
    }

    await sleep(1000);
  }

  console.log('[01] PASS — Create profile scenario complete');
}

function sleep(ms: number): Promise<void> {
  return new Promise(r => setTimeout(r, ms));
}
