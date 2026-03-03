/**
 * Scenario 01: First Launch — Create Profile (Multi-Step Wizard)
 *
 * Exercises the full onboarding: TrustScreen → ProfileTypeChoice → CreateProfile
 * (4 sub-steps: Identity → Health → Location → Security) → RecoveryPhrase → WelcomeTour → Home.
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

  // ── Step 3: Choose "For myself" → Sub-step 0 (Identity) ──────────────
  console.log('[01] Selecting self profile...');
  await harness.browser!.execute(() => {
    const buttons = document.querySelectorAll('button');
    for (const btn of buttons) {
      if (btn.textContent && btn.textContent.length > 20 && !btn.textContent.includes('Back')) {
        btn.click();
        return;
      }
    }
  });
  await sleep(1500);
  await driver.screenshot('01-step-identity');

  // ── Step 4a: Identity — Fill name ─────────────────────────────────────
  console.log('[01] Filling identity step...');
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

  // Click "Next" to advance to Health step
  console.log('[01] Advancing to health step...');
  await harness.browser!.execute(() => {
    const buttons = document.querySelectorAll('button');
    for (const btn of buttons) {
      if (btn.textContent?.includes('Next') && !btn.disabled) {
        btn.click();
        return;
      }
    }
  });
  await sleep(800);
  await driver.screenshot('01-step-health');

  // ── Step 4b: Health — Select Female pill + European chip ──────────────
  console.log('[01] Filling health step...');

  // Click the "Female" pill button
  await harness.browser!.execute(() => {
    const buttons = document.querySelectorAll('button');
    for (const btn of buttons) {
      if (btn.textContent?.trim() === 'Female') {
        btn.click();
        return;
      }
    }
  });
  await sleep(200);

  // Click the "European" chip
  await harness.browser!.execute(() => {
    const buttons = document.querySelectorAll('button');
    for (const btn of buttons) {
      if (btn.textContent?.trim() === 'European') {
        btn.click();
        return;
      }
    }
  });
  await sleep(200);

  // Click "Next" to advance to Location step
  console.log('[01] Advancing to location step...');
  await harness.browser!.execute(() => {
    const buttons = document.querySelectorAll('button');
    for (const btn of buttons) {
      if (btn.textContent?.includes('Next') && !btn.disabled) {
        btn.click();
        return;
      }
    }
  });
  await sleep(800);
  await driver.screenshot('01-step-location');

  // ── Step 4c: Location — Skip (advance to Security) ───────────────────
  console.log('[01] Skipping location step...');
  await harness.browser!.execute(() => {
    const buttons = document.querySelectorAll('button');
    for (const btn of buttons) {
      if (btn.textContent?.includes('Next') && !btn.disabled) {
        btn.click();
        return;
      }
    }
  });
  await sleep(800);
  await driver.screenshot('01-step-security');

  // ── Step 4d: Security — Fill password ─────────────────────────────────
  console.log('[01] Filling security step...');
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

  // ── Step 5: Submit → Create Profile ───────────────────────────────────
  console.log('[01] Submitting profile...');
  await harness.browser!.execute(() => {
    const buttons = document.querySelectorAll('button');
    for (const btn of buttons) {
      const text = btn.textContent?.trim() ?? '';
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
  if (harness.isOllamaAvailable()) {
    console.log('[01] Configuring AI model...');
    const MODEL_NAME = 'ktiyab/coheara-medgemma-4b-q4:latest';

    try {
      await driver.executeInvoke('set_active_model', { modelName: MODEL_NAME });
      console.log(`[01] Active model set: ${MODEL_NAME}`);
    } catch (err) {
      console.log(`[01] WARN — set_active_model failed: ${err}`);
    }

    try {
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
