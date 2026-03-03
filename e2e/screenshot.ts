#!/usr/bin/env tsx
/**
 * E2E-01 Brick 4: Ad-hoc screenshot tool.
 *
 * Usage:
 *   npx tsx e2e/screenshot.ts                   → screenshot current state
 *   npx tsx e2e/screenshot.ts --screen home     → navigate to home, screenshot
 *   npx tsx e2e/screenshot.ts --screen chat     → navigate to chat, screenshot
 *   npx tsx e2e/screenshot.ts --all             → screenshot all main screens
 *
 * Output: e2e/results/{name}.png — file path printed to stdout.
 */

import { TestHarness } from './harness.js';
import { CohearaDriver, type Screen } from './driver.js';

const ALL_SCREENS: Screen[] = ['home', 'chat', 'history', 'documents', 'timeline', 'settings'];

async function main(): Promise<void> {
  const args = process.argv.slice(2);
  const screenArg = args.includes('--screen') ? args[args.indexOf('--screen') + 1] as Screen : null;
  const allMode = args.includes('--all');

  // Check if harness is already running (persistent mode)
  const existing = TestHarness.isRunning();
  let harness: TestHarness;
  let ownHarness = false;

  if (existing.running && existing.display && existing.driverPort) {
    console.log(`[screenshot] Reusing existing harness (display=:${existing.display}, port=${existing.driverPort})`);
    harness = new TestHarness(existing.display, existing.driverPort);
    // Connect to existing instance (skip full setup)
    harness.browser = await (await import('webdriverio')).remote({
      hostname: 'localhost',
      port: existing.driverPort,
      capabilities: {
        'tauri:options': {
          application: '/dev/null', // Ignored for existing session
        },
      } as WebdriverIO.Capabilities,
    });
  } else {
    console.log('[screenshot] Starting fresh harness...');
    harness = new TestHarness();
    await harness.setup();
    ownHarness = true;
  }

  try {
    const driver = new CohearaDriver(harness);
    const paths: string[] = [];

    if (allMode) {
      for (const screen of ALL_SCREENS) {
        await driver.navigate(screen);
        await sleep(1000); // Let animations settle
        const path = await driver.screenshot(`screen-${screen}`);
        paths.push(path);
      }
    } else if (screenArg) {
      await driver.navigate(screenArg);
      await sleep(1000);
      const path = await driver.screenshot(`screen-${screenArg}`);
      paths.push(path);
    } else {
      await sleep(500);
      const path = await driver.screenshot('current-view');
      paths.push(path);
    }

    // Print paths for Claude to Read
    console.log('\n--- SCREENSHOT PATHS ---');
    for (const p of paths) {
      console.log(p);
    }
  } finally {
    if (ownHarness) {
      await harness.teardown();
    }
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise(r => setTimeout(r, ms));
}

main().catch(e => {
  console.error('[screenshot] Fatal error:', e);
  process.exit(1);
});
