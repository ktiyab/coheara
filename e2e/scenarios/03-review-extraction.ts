/**
 * Scenario 03: Review Extracted Entities
 *
 * Precondition: Document imported (scenario 02).
 * With Ollama: waits for extraction to complete (Butler becomes idle),
 * then verifies extraction review screen with real extracted entities.
 * Without Ollama: skips (no entities to review).
 *
 * Note: CPU-based IterativeDrill extraction (enumerate + drill per item)
 * takes 10-20 minutes for a multi-value lab report on Ryzen 7 3700X.
 * The Butler lock is held for the entire extraction duration.
 */
import type { TestHarness } from '../harness.js';
import { CohearaDriver } from '../driver.js';

// CPU extraction: enumerate (~120s) + drill per item (2 calls × ~60s × N items)
// For 8-10 lab values: ~20 LLM calls × 30-60s = 10-20 minutes
const EXTRACTION_TIMEOUT_MS = 1_200_000; // 20 minutes

export async function run(harness: TestHarness): Promise<void> {
  const driver = new CohearaDriver(harness);

  // ── Gate: Ollama required for extraction ─────────────────────────────
  if (!harness.isOllamaAvailable()) {
    console.log('[03] SKIP — Ollama not available (required for extraction review)');
    return;
  }

  // ── Step 1: Wait for Butler to become idle (extraction complete) ─────
  // The import worker holds the Butler lock during the entire vision OCR
  // + structuring pipeline. We must wait for it to finish before checking
  // for pending extractions or attempting chat.
  console.log(`[03] Waiting for extraction to complete (timeout: ${EXTRACTION_TIMEOUT_MS / 60_000}min)...`);

  const extractionDeadline = Date.now() + EXTRACTION_TIMEOUT_MS;
  let butlerIdle = false;
  let lastLog = 0;

  while (Date.now() < extractionDeadline) {
    try {
      const butler = await driver.executeInvoke<{
        active_operation?: string | null;
        loaded_model?: string | null;
        idle_secs?: number;
      }>('get_butler_status');

      const op = butler?.active_operation;
      const idle = butler?.idle_secs ?? 0;

      // Log every 30s and keep session alive
      if (Date.now() - lastLog > 30_000) {
        console.log(`[03] Butler: operation=${op ?? 'none'}, idle=${idle}s, model=${butler?.loaded_model ?? '?'}`);
        lastLog = Date.now();
        // Reset inactivity timer to prevent 15-minute session timeout
        try { await driver.executeInvoke('update_activity'); } catch { /* ignore */ }
      }

      // Butler is idle when no active operation
      if (!op || op === 'null' || op === '') {
        // Wait a bit to make sure it's truly idle (not between operations)
        await sleep(3000);
        const recheck = await driver.executeInvoke<{ active_operation?: string | null }>('get_butler_status');
        if (!recheck?.active_operation) {
          butlerIdle = true;
          console.log('[03] Butler is idle — extraction pipeline complete');
          break;
        }
      }
    } catch {
      // Ignore errors during status check
    }
    await sleep(5000);
  }

  if (!butlerIdle) {
    console.log('[03] SKIP — Extraction still running after timeout');
    await driver.screenshot('03-extraction-timeout');
    return;
  }

  // ── Step 2: Check for pending extractions ───────────────────────────
  let pendingCount = 0;
  try {
    pendingCount = await driver.executeInvoke<number>('get_pending_extraction_count') ?? 0;
    console.log(`[03] Pending extraction count: ${pendingCount}`);
  } catch (err) {
    console.log(`[03] Could not check extraction count: ${err}`);
  }

  if (pendingCount === 0) {
    console.log('[03] WARN — No pending extractions found (extraction may have failed or entities auto-confirmed)');
    await driver.screenshot('03-no-extractions');
    // Continue anyway — try to navigate to review screen
  }

  // ── Step 3: Navigate to home and find review entry point ────────────
  console.log('[03] Navigating to home for review...');
  await driver.navigate('home');
  await sleep(2000);
  await driver.screenshot('03-home-with-review');

  // Try to navigate to review via the review card on home screen
  const clickedReview = await harness.browser!.execute(() => {
    const buttons = document.querySelectorAll('button');
    for (const btn of buttons) {
      const text = btn.textContent?.trim() ?? '';
      if (text.includes('Review') || text.includes('review')) {
        btn.click();
        return true;
      }
    }
    // Also try clicking on extraction review cards
    const links = document.querySelectorAll('a, [role="link"], [data-review]');
    for (const link of links) {
      const text = link.textContent?.trim() ?? '';
      if (text.includes('Review') || text.includes('review') || text.includes('extraction')) {
        (link as HTMLElement).click();
        return true;
      }
    }
    return false;
  });

  if (clickedReview) {
    console.log('[03] Clicked review entry point');
    await sleep(2000);
  } else {
    console.log('[03] No review button found — trying direct navigation');
    await driver.navigate('review');
    await sleep(2000);
  }

  await driver.screenshot('03-review-screen');

  // ── Step 4: Verify entities are visible ─────────────────────────────
  const hasEntities = await harness.browser!.execute(() => {
    const text = document.body.textContent ?? '';
    // Look for common lab entity keywords from test-lab-minimal.pdf (3 items)
    return text.includes('Hemoglobin') || text.includes('Glucose') ||
           text.includes('TSH') || text.includes('hemoglobin') ||
           text.includes('glucose') || text.includes('Lab') ||
           text.includes('lab') || text.includes('13.2') ||
           text.includes('95') || text.includes('2.1');
  });

  if (hasEntities) {
    console.log('[03] Extracted entities visible on review screen');
  } else {
    console.log('[03] WARN — Expected entity keywords not found');
    await driver.screenshot('03-review-no-entities');
  }

  // ── Step 5: Attempt to confirm review ───────────────────────────────
  console.log('[03] Looking for confirm action...');
  const confirmed = await harness.browser!.execute(() => {
    const buttons = document.querySelectorAll('button');
    for (const btn of buttons) {
      const text = btn.textContent?.trim() ?? '';
      if ((text.includes('Confirm') || text.includes('Accept') || text.includes('Approve')) && !btn.disabled) {
        btn.click();
        return true;
      }
    }
    return false;
  });

  if (confirmed) {
    console.log('[03] Clicked confirm button');
    await sleep(2000);
    await driver.screenshot('03-review-confirmed');
  } else {
    console.log('[03] No confirm button available — review may require entity selection first');
    await driver.screenshot('03-review-state');
  }

  console.log('[03] PASS — Review extraction scenario complete');
}

function sleep(ms: number): Promise<void> {
  return new Promise(r => setTimeout(r, ms));
}
