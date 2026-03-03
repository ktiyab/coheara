/**
 * E2E-01 Brick 5: Suite Runner — Orchestrates all test scenarios.
 *
 * Usage: cd e2e && npx tsx run-suite.ts [--scenario 01] [--quick]
 *
 * Flags:
 *   --scenario 01  Run a single scenario by ID
 *   --quick        Run only UI-only scenarios (no Ollama: 01, 02, 05)
 *
 * Runs scenarios in sequence (01→05). Each scenario gets a try/catch
 * so failures don't block subsequent scenarios. Produces a JSON report.
 */
import { writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { TestHarness } from './harness.js';

import { run as scenario01 } from './scenarios/01-create-profile.js';
import { run as scenario02 } from './scenarios/02-import-document.js';
import { run as scenario03 } from './scenarios/03-review-extraction.js';
import { run as scenario04 } from './scenarios/04-chat-with-data.js';
import { run as scenario05 } from './scenarios/05-navigation.js';

// ── Types ────────────────────────────────────────────────────────────────────

interface ScenarioResult {
  id: string;
  name: string;
  status: 'pass' | 'fail' | 'skip';
  durationMs: number;
  error?: string;
}

interface SuiteReport {
  timestamp: string;
  totalDurationMs: number;
  ollamaAvailable: boolean;
  scenarios: ScenarioResult[];
  summary: { pass: number; fail: number; skip: number };
}

// ── Scenario Registry ────────────────────────────────────────────────────────

const SCENARIOS = [
  { id: '01', name: 'Create Profile', fn: scenario01, needsOllama: false },
  { id: '02', name: 'Import Document', fn: scenario02, needsOllama: false },
  { id: '03', name: 'Review Extraction', fn: scenario03, needsOllama: true },
  { id: '04', name: 'Chat With Data', fn: scenario04, needsOllama: true },
  { id: '05', name: 'Navigation', fn: scenario05, needsOllama: false },
];

// ── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  const args = process.argv.slice(2);
  const singleScenario = args.includes('--scenario') ? args[args.indexOf('--scenario') + 1] : null;
  const quickMode = args.includes('--quick');

  const harness = new TestHarness();
  const results: ScenarioResult[] = [];
  const suiteStart = Date.now();

  try {
    // ── Setup ─────────────────────────────────────────────────────────────
    await harness.setup();

    // Reset app data for a clean slate (fresh DB, no existing profiles)
    await harness.resetApp();

    const mode = quickMode ? 'QUICK (UI-only)' : singleScenario ? `SCENARIO ${singleScenario}` : 'FULL';
    console.log('\n' + '='.repeat(60));
    console.log(`  E2E TEST SUITE — ${mode}`);
    console.log(`  Ollama: ${harness.isOllamaAvailable() ? 'Available' : 'Not available'}`);
    if (quickMode) console.log('  Skipping Ollama-dependent scenarios (03, 04)');
    console.log('='.repeat(60) + '\n');

    // ── Run Scenarios ─────────────────────────────────────────────────────
    let scenariosToRun = singleScenario
      ? SCENARIOS.filter(s => s.id === singleScenario)
      : SCENARIOS;

    if (quickMode) {
      scenariosToRun = scenariosToRun.filter(s => !s.needsOllama);
    }

    for (const scenario of scenariosToRun) {
      console.log(`\n${'─'.repeat(50)}`);
      console.log(`  Scenario ${scenario.id}: ${scenario.name}`);
      console.log(`${'─'.repeat(50)}\n`);

      const start = Date.now();
      try {
        await scenario.fn(harness);
        results.push({
          id: scenario.id,
          name: scenario.name,
          status: 'pass',
          durationMs: Date.now() - start,
        });
      } catch (err) {
        const errorMsg = err instanceof Error ? err.message : String(err);
        console.error(`[${scenario.id}] FAIL: ${errorMsg}`);

        // Diagnostic screenshot on failure
        try {
          await harness.screenshot(`fail-${scenario.id}`);
        } catch { /* screenshot may fail too */ }

        results.push({
          id: scenario.id,
          name: scenario.name,
          status: 'fail',
          durationMs: Date.now() - start,
          error: errorMsg,
        });
      }
    }
  } finally {
    // ── Teardown ──────────────────────────────────────────────────────────
    await harness.teardown();
  }

  // ── Report ──────────────────────────────────────────────────────────────
  const totalDuration = Date.now() - suiteStart;
  const summary = {
    pass: results.filter(r => r.status === 'pass').length,
    fail: results.filter(r => r.status === 'fail').length,
    skip: results.filter(r => r.status === 'skip').length,
  };

  const report: SuiteReport = {
    timestamp: new Date().toISOString(),
    totalDurationMs: totalDuration,
    ollamaAvailable: harness.isOllamaAvailable(),
    scenarios: results,
    summary,
  };

  // Write JSON report
  const reportPath = join(resolve(import.meta.dirname), 'results', 'report.json');
  writeFileSync(reportPath, JSON.stringify(report, null, 2));

  // Console summary
  console.log('\n' + '='.repeat(60));
  console.log('  RESULTS');
  console.log('='.repeat(60));

  for (const r of results) {
    const icon = r.status === 'pass' ? 'PASS' : r.status === 'fail' ? 'FAIL' : 'SKIP';
    const duration = `${(r.durationMs / 1000).toFixed(1)}s`;
    console.log(`  [${icon}] ${r.id} ${r.name} (${duration})`);
    if (r.error) console.log(`         ${r.error}`);
  }

  console.log(`\n  Total: ${summary.pass} passed, ${summary.fail} failed, ${summary.skip} skipped`);
  console.log(`  Duration: ${(totalDuration / 1000).toFixed(1)}s`);
  console.log(`  Report: ${reportPath}`);
  console.log('='.repeat(60) + '\n');

  // Exit with failure code if any scenario failed
  if (summary.fail > 0) {
    process.exitCode = 1;
  }
}

main().catch(err => {
  console.error('Suite runner crashed:', err);
  process.exitCode = 1;
});
