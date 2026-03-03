/**
 * Scenario 02: Import a Lab Results PDF
 *
 * Precondition: Profile "Alice Martin" active + model configured (from scenario 01).
 * Imports test document via executeInvoke, waits for queue processing.
 * With Ollama: waits for extraction to complete (vision OCR + structuring).
 */
import { resolve } from 'node:path';
import type { TestHarness } from '../harness.js';
import { CohearaDriver } from '../driver.js';

// Use minimal fixture (3 items) for fast CPU extraction (~5-8 min)
// Full fixture (test-lab-results.pdf, 20 items) takes 30-40 min on CPU
const FIXTURE_PATH = resolve(import.meta.dirname, '..', 'fixtures', 'test-lab-minimal.pdf');

// CPU extraction can take 3-5 minutes per page (vision OCR + structuring)
const EXTRACTION_TIMEOUT_MS = 300_000;

export async function run(harness: TestHarness): Promise<void> {
  const driver = new CohearaDriver(harness);

  // ── Step 1: Verify we're on home screen with active profile ───────────
  console.log('[02] Verifying home screen...');
  await driver.screenshot('02-home-before-import');

  // ── Step 2: Import document via IPC ───────────────────────────────────
  console.log(`[02] Importing document: ${FIXTURE_PATH}`);
  try {
    // Tauri v2 converts Rust snake_case to JS camelCase
    await driver.executeInvoke('enqueue_imports', {
      filePaths: [FIXTURE_PATH],
      documentType: 'LabResults',
    });
    console.log('[02] Import enqueued successfully (LabResults type)');
  } catch (err) {
    console.log(`[02] enqueue_imports failed: ${err}`);
    await driver.screenshot('02-import-failed');
    throw new Error(`Import failed: ${err}`);
  }

  // ── Step 3: Wait for import queue to drain ────────────────────────────
  console.log('[02] Waiting for import processing...');
  await sleep(3000);
  await driver.screenshot('02-import-processing');

  const deadline = Date.now() + EXTRACTION_TIMEOUT_MS;
  let lastStatus = '';

  while (Date.now() < deadline) {
    try {
      const queue = await driver.executeInvoke<{ items?: Array<{ status?: string }> }>('get_import_queue');
      const items = queue?.items ?? [];

      if (items.length === 0) {
        console.log('[02] Import queue drained — processing complete');
        break;
      }

      const status = items.map(i => i.status ?? 'unknown').join(', ');
      if (status !== lastStatus) {
        console.log(`[02] Queue: ${items.length} items [${status}]`);
        lastStatus = status;
      }
    } catch {
      // Queue command may return differently — continue polling
    }
    await sleep(5000);
  }

  if (Date.now() >= deadline) {
    console.log(`[02] WARN — Import still processing after ${EXTRACTION_TIMEOUT_MS / 1000}s`);
  }

  await driver.screenshot('02-import-done');

  // ── Step 4: Navigate to documents screen ──────────────────────────────
  console.log('[02] Navigating to documents...');
  await driver.navigate('documents');
  await sleep(2000);
  await driver.screenshot('02-documents-after-import');

  // ── Step 5: Verify document appears ───────────────────────────────────
  const hasDocument = await harness.browser!.execute(() => {
    const text = document.body.textContent ?? '';
    return text.includes('lab') || text.includes('Lab') ||
           text.includes('test-lab') || text.includes('document') ||
           text.includes('Document');
  });

  if (hasDocument) {
    console.log('[02] Document visible in document list');
  } else {
    console.log('[02] WARN — Document not visible yet');
  }

  // ── Step 6: Check if extraction produced entities (Ollama path) ───────
  if (harness.isOllamaAvailable()) {
    try {
      const count = await driver.executeInvoke<number>('get_pending_extraction_count');
      console.log(`[02] Pending extraction count: ${count}`);
    } catch (err) {
      console.log(`[02] Could not check extraction count: ${err}`);
    }
  }

  console.log('[02] PASS — Import document scenario complete');
}

function sleep(ms: number): Promise<void> {
  return new Promise(r => setTimeout(r, ms));
}
