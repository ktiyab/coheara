#!/usr/bin/env tsx
/**
 * E2E-01 Brick 4: Persistent harness keep-alive.
 *
 * Starts the full test harness and keeps it running until Ctrl+C.
 * Other scripts (screenshot.ts, test scenarios) detect the PID file
 * and reuse the running instance for ~2s screenshot loops.
 *
 * Usage:
 *   npx tsx e2e/keep-alive.ts
 *   # Then in another terminal:
 *   npx tsx e2e/screenshot.ts --screen home    (instant, reuses harness)
 */

import { TestHarness } from './harness.js';

async function main(): Promise<void> {
  const harness = new TestHarness();

  // Clean shutdown on Ctrl+C / SIGTERM
  const shutdown = async () => {
    console.log('\n[keep-alive] Shutting down...');
    await harness.teardown();
    process.exit(0);
  };

  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);

  await harness.setup();
  console.log('\n[keep-alive] Harness is running. Press Ctrl+C to stop.');
  console.log(`[keep-alive] Display=:${harness.display}, Port=${harness.driverPort}`);
  console.log('[keep-alive] Run screenshot.ts in another terminal to take screenshots.\n');

  // Keep alive — sleep in 10s intervals
  while (true) {
    await new Promise(r => setTimeout(r, 10_000));
  }
}

main().catch(e => {
  console.error('[keep-alive] Fatal error:', e);
  process.exit(1);
});
