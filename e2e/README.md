# Coheara E2E Testing Infrastructure

Full-stack visual testing for Coheara — build, launch, interact, screenshot, verify — with zero GUI dependency and zero app source code changes.

**For AI operators (Claude, etc.):** This system gives you eyes and hands. You can see every screen, fill forms, click buttons, read text, and take screenshots to verify your work visually.

**For humans:** This runs the real Coheara app headlessly (no monitor needed), automates user flows, and captures screenshots at every step.

---

## Table of Contents

| Section | What You'll Find |
|---------|-----------------|
| [Quick Start](#quick-start) | Run the suite in one command |
| [Architecture](#architecture) | How the pieces fit together |
| [Commands](#commands) | Every way to run e2e tests |
| [File Map](#file-map) | What each file does |
| [TestHarness API](#testharness-api) | Lifecycle management class |
| [CohearaDriver API](#cohearadriver-api) | WebDriver wrapper with all methods |
| [Scenarios](#scenarios) | What each test scenario covers |
| [Test Fixtures](#test-fixtures) | PDF documents used for testing |
| [Screenshots](#screenshots) | Where outputs go and how to read them |
| [Writing New Scenarios](#writing-new-scenarios) | How to add tests |
| [Environment Variables](#environment-variables) | Headless rendering config |
| [Data Paths](#data-paths) | Where the app stores data during tests |
| [System Dependencies](#system-dependencies) | Required packages |
| [Troubleshooting](#troubleshooting) | Common errors and fixes |

---

## Quick Start

```bash
# From project root — run the full 5-scenario suite (~20min with Ollama)
./dev.sh e2e

# Quick mode — UI-only scenarios, no Ollama needed (~2-3min)
./dev.sh e2e:quick

# Single scenario
./dev.sh e2e --scenario 01

# Or from e2e/ directory
cd e2e && npx tsx run-suite.ts
cd e2e && npx tsx run-suite.ts --quick
cd e2e && npx tsx run-suite.ts --scenario 01
```

**What happens:**
1. Starts Xvfb virtual display on `:99`
2. Builds frontend (`npm run build`) if Svelte/TS/i18n sources changed
3. Builds Linux binary (`cargo build`) if Rust or frontend sources changed
4. Starts tauri-driver on port 4444
5. Launches Coheara via WebDriver session
6. Resets app data (fresh DB, no existing profiles)
7. Runs scenarios (all 5, quick mode: 01+02+05, or single)
8. Saves screenshots to `e2e/results/*.png`
9. Writes JSON report to `e2e/results/report.json`
10. Tears down all processes

**Quick mode output (~2-3 min):**
```
============================================================
  E2E TEST SUITE — QUICK (UI-only)
  Ollama: Available
  Skipping Ollama-dependent scenarios (03, 04)
============================================================

  [PASS] 01 Create Profile (151.6s)
  [PASS] 02 Import Document (9.7s)
  [PASS] 05 Navigation (18.0s)

  Total: 3 passed, 0 failed, 0 skipped
  Duration: 268.3s
============================================================
```

**Full suite output (~20 min with Ollama):**
```
============================================================
  E2E TEST SUITE — FULL
  Ollama: Available
============================================================

  [PASS] 01 Create Profile (108.6s)
  [PASS] 02 Import Document (6.7s)
  [PASS] 03 Review Extraction (669.4s)
  [PASS] 04 Chat With Data (11.5s)
  [PASS] 05 Navigation (19.8s)

  Total: 5 passed, 0 failed, 0 skipped
  Duration: 1161.1s
============================================================
```

---

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│  Test Script (e2e/*.ts)                                  │
│  Uses: TestHarness + CohearaDriver                       │
└──────────────┬───────────────────────────────────────────┘
               │ WebdriverIO (HTTP)
               ▼
┌──────────────────────────────────────────────────────────┐
│  tauri-driver (:4444)                                    │
│  W3C WebDriver ↔ WebKitGTK automation                    │
│  Launches app binary via tauri:options.application        │
└──────────────┬───────────────────────────────────────────┘
               │ WebKitGTK IPC
               ▼
┌──────────────────────────────────────────────────────────┐
│  Coheara (real app, unmodified)                          │
│  Svelte frontend + Rust backend                          │
│  Running on Xvfb DISPLAY=:99 (1280x800)                  │
└──────────────────────────────────────────────────────────┘
               │
               ▼
┌──────────────────────────────────────────────────────────┐
│  Xvfb (X Virtual Framebuffer)                            │
│  Virtual display — no physical monitor needed             │
│  ImageMagick `import` captures window → PNG               │
└──────────────────────────────────────────────────────────┘
```

**Key insight:** WebDriver handles DOM interaction (click, type, read text, execute JavaScript). But WebDriver's `takeScreenshot()` fails when WebKitGTK compositing is disabled. So screenshots use ImageMagick's `import -window` command to capture directly from the X11 display.

**Isolation:** This directory is completely isolated from app source code. It has its own `package.json`, `node_modules/`, and `tsconfig.json`. The app binary is the only shared artifact.

---

## Commands

### From project root (via dev.sh)

```bash
./dev.sh e2e                        # Run full 5-scenario suite (~20min)
./dev.sh e2e:quick                  # UI-only scenarios — no Ollama (~2-3min)
./dev.sh e2e --scenario 01          # Run single scenario by ID
./dev.sh e2e:screenshot             # Screenshot current app state
./dev.sh e2e:screenshot --screen home  # Navigate to home, screenshot
./dev.sh e2e:screenshot --all       # Screenshot all 5 main screens
./dev.sh e2e:keepalive              # Start persistent harness (Ctrl+C to stop)
```

### From e2e/ directory (via npx tsx)

```bash
cd e2e

npx tsx run-suite.ts                # Full suite
npx tsx run-suite.ts --quick        # UI-only scenarios (01, 02, 05)
npx tsx run-suite.ts --scenario 01  # Single scenario (01-05)
npx tsx run-suite.ts --scenario 04  # Just the chat scenario

npx tsx screenshot.ts               # Current view
npx tsx screenshot.ts --screen chat # Navigate + capture
npx tsx screenshot.ts --all         # All screens

npx tsx keep-alive.ts               # Persistent harness
```

### Which mode to use?

| Change Type | Command | Why |
|-------------|---------|-----|
| UI components, i18n, styling | `./dev.sh e2e:quick` | No AI pipeline changes, fast feedback |
| AI pipeline, extraction, chat | `./dev.sh e2e` | Full suite validates Ollama-dependent flows |
| Single screen change | `./dev.sh e2e --scenario 01` | Targeted verification |
| Quick visual check | `./dev.sh e2e:screenshot --all` | Instant screenshots, no scenario logic |

### Fast screenshot loop (two terminals)

```bash
# Terminal 1: Start persistent harness (one-time ~30s setup)
cd e2e && npx tsx keep-alive.ts

# Terminal 2: Instant screenshots (~2s each, reuses running harness)
cd e2e && npx tsx screenshot.ts --screen home
cd e2e && npx tsx screenshot.ts --screen settings
cd e2e && npx tsx screenshot.ts --all
```

---

## File Map

```
e2e/
├── README.md              ← This file
├── package.json           ← Dependencies (webdriverio, tsx, typescript)
├── package-lock.json
├── tsconfig.json          ← ES2022, ESNext modules, strict mode
│
├── harness.ts             ← TestHarness class (lifecycle management)
├── driver.ts              ← CohearaDriver class (WebDriver wrapper)
├── screenshot.ts          ← Ad-hoc screenshot CLI tool
├── keep-alive.ts          ← Persistent harness server
├── run-suite.ts           ← Suite runner + JSON report generator
│
├── scenarios/
│   ├── 01-create-profile.ts      ← Multi-step wizard onboarding (no Ollama)
│   ├── 02-import-document.ts     ← PDF import via IPC (no Ollama)
│   ├── 03-review-extraction.ts   ← Entity review (needs Ollama)
│   ├── 04-chat-with-data.ts      ← Chat UI + AI response (needs Ollama)
│   └── 05-navigation.ts          ← Visit all screens (no Ollama)
│
├── fixtures/
│   ├── test-lab-results.pdf      ← CBC, metabolic, lipid, thyroid panels
│   └── test-prescription.pdf     ← 3 medications with dosing
│
├── results/               ← Output directory (git-ignored except .gitkeep)
│   ├── .gitkeep
│   ├── *.png              ← Screenshots (generated)
│   └── report.json        ← Suite results (generated)
│
├── .data/                 ← WebKit storage during tests (git-ignored)
└── node_modules/          ← Dependencies (git-ignored)
```

---

## TestHarness API

**File:** `e2e/harness.ts`
**Import:** `import { TestHarness } from './harness.js';`

Manages the complete lifecycle: Xvfb display, binary build, tauri-driver, WebDriver session, and cleanup.

### Constructor

```typescript
const harness = new TestHarness(display?: number, driverPort?: number);
// Defaults: display = 99, driverPort = 4444
```

### Properties

| Property | Type | Description |
|----------|------|-------------|
| `display` | `number` | Xvfb display number (e.g., 99 → `:99`) |
| `driverPort` | `number` | tauri-driver HTTP port |
| `browser` | `Browser \| null` | WebdriverIO browser instance (null before setup) |
| `hasOllama` | `boolean` | Whether Ollama is running on localhost:11434 |
| `isReady` | `boolean` | True after successful `setup()` |

### Methods

#### `setup(): Promise<void>`

Full initialization sequence:
1. Find free X display (scans `:99` down to `:90`)
2. Start Xvfb with resolution 1280x800x24
3. Build Linux binary if source files are newer (up to 10min timeout)
4. Start tauri-driver on configured port (15s ready timeout)
5. Create WebDriver session (tauri-driver launches app via capability)
6. Wait 8s for SvelteKit hydration
7. Detect Ollama availability

#### `teardown(): Promise<void>`

Kill all processes, close browser session, clean up lock files and PID file.

#### `screenshot(name: string): Promise<string>`

Capture X11 window screenshot via ImageMagick. Returns absolute path to PNG file at `e2e/results/{name}.png`. Uses `xdotool search` to find the window ID.

#### `resetApp(): Promise<void>`

1. Delete WebDriver session (kills the app)
2. Remove `~/Coheara-dev/profiles/` (app data for debug builds)
3. Remove `e2e/.data/com.coheara.app/` (WebKit cache)
4. Reconnect browser (fresh app instance, empty DB)
5. Wait 8s for hydration

#### `isOllamaAvailable(): boolean`

Returns cached Ollama detection result from `setup()`.

#### `static isRunning(): { running: boolean; display?: number; driverPort?: number }`

Check if `.harness.pid` exists and process is alive. Used by `screenshot.ts` for harness reuse.

### Constants

| Name | Value | Purpose |
|------|-------|---------|
| `BINARY_PATH` | `/tmp/coheara-build/x86_64-unknown-linux-gnu/debug/coheara` | Compiled app |
| `BUILD_TARGET_DIR` | `/tmp/coheara-build` | Cargo output |
| `LINUX_TARGET` | `x86_64-unknown-linux-gnu` | Build triple |
| `RESULTS_DIR` | `e2e/results` | Screenshot output |
| `CARGO` | `/root/.cargo/bin/cargo` | Cargo binary |
| `PAGE_LOAD_WAIT_MS` | `8000` | SvelteKit hydration wait |
| `DRIVER_READY_TIMEOUT_MS` | `15000` | tauri-driver startup |
| `BUILD_TIMEOUT_MS` | `600000` | Cargo build (10 min) |

---

## CohearaDriver API

**File:** `e2e/driver.ts`
**Import:** `import { CohearaDriver, type Screen } from './driver.js';`

Domain-specific wrapper over raw WebDriver. Provides navigation, form filling, Tauri IPC access, and assertions.

### Constructor

```typescript
const driver = new CohearaDriver(harness: TestHarness);
// Requires harness.browser to be connected
```

### Screen Type

```typescript
type Screen =
  | 'home' | 'chat' | 'history' | 'documents' | 'document-detail'
  | 'review' | 'timeline' | 'settings' | 'ai-settings' | 'privacy'
  | 'companion' | 'profiles' | 'profiles-create';
```

### Navigation

| Method | Description |
|--------|-------------|
| `navigate(screen, params?)` | Set `window.location.hash`, wait 500ms |
| `waitForScreen(screen, timeoutMs=10000)` | Wait for screen-specific DOM selector or hash match |
| `getCurrentScreen()` | Read current hash → screen name |

### Form Interaction

| Method | Description |
|--------|-------------|
| `type(selector, text)` | Clear input, set value, dispatch `input` + `change` events for Svelte reactivity |
| `click(selector)` | Wait for element, click |
| `clickByText(text)` | XPath search for text content, click first match |
| `selectRadio(name, value)` | Find radio by name+value attribute, click parent label |
| `toggleCheckbox(selector)` | Click checkbox element |
| `selectDropdown(selector, value)` | Native WebDriver select by value attribute |
| `fillForm(fields: FormField[])` | Batch fill: iterates fields, dispatches by type |

### Data Access

| Method | Description |
|--------|-------------|
| `executeInvoke<T>(command, args?)` | Call Tauri IPC directly: `window.__TAURI_INTERNALS__.invoke(cmd, args)` |
| `getText(selector)` | Get visible text from element |
| `getInputValue(selector)` | Get input field value |
| `isVisible(selector)` | Check if element is displayed (returns false if not found) |
| `waitForText(text, timeoutMs=30000)` | Poll `document.body.textContent` until text appears |
| `waitForElement(selector, timeoutMs=10000)` | Wait for element to exist in DOM |

### Screenshots

| Method | Description |
|--------|-------------|
| `screenshot(name)` | Delegates to `harness.screenshot()` |

### Assertions

All assertion methods take a screenshot on failure before throwing.

| Method | Description |
|--------|-------------|
| `assertVisible(selector, message?)` | Throw if element not visible |
| `assertNotVisible(selector, message?)` | Throw if element IS visible |
| `assertText(selector, expected)` | Throw if element doesn't contain text |
| `assertScreenIs(screen)` | Throw if current hash doesn't match screen |
| `assertBodyContains(text)` | Throw if body text doesn't include string |

### Raw Access

```typescript
driver.raw  // Returns underlying WebdriverIO Browser instance
```

---

## Scenarios

Each scenario exports `run(harness: TestHarness): Promise<void>`.

### 01 — Create Profile (Multi-Step Wizard)

**What it tests:** Full onboarding from first launch to home screen, including the 4-step profile creation wizard (UX-04).

**Flow:** Trust Screen → "Create your MedAI Vault" → Self Profile → **Step 1 Identity** (name: Alice Martin) → **Step 2 Health** (sex: Female pill, ethnicity: European chip) → **Step 3 Location** (skip) → **Step 4 Security** (password: TestPassword42!) → Submit → Recovery Phrase (12 words, skip verify) → Welcome Tour (skip) → Home Screen → Configure AI model.

**Duration:** ~100-150s (crypto: Argon2 + AES-256-GCM, plus 4-step navigation).

**Ollama required:** No (profile creation is CPU-only).

**Screenshots:** `01-trust-screen.png`, `01-profile-type.png`, `01-step-identity.png`, `01-step-health.png`, `01-step-location.png`, `01-step-security.png`, `01-post-create.png`, `01-recovery-phrase.png`, `01-welcome-tour.png`, `01-home-screen.png`

### 02 — Import Document

**What it tests:** PDF import via Tauri IPC and document list verification.

**Precondition:** Profile active (from scenario 01).

**Flow:** Verify home → enqueue `test-lab-results.pdf` via `executeInvoke('enqueue_imports')` → poll import queue (120s timeout for OCR) → navigate to Documents → verify document visible.

**Screenshots:** `02-home-before-import.png`, `02-import-processing.png`, `02-documents-after-import.png`

### 03 — Review Extraction

**What it tests:** Extracted entity review after document processing.

**Gate:** Skipped if Ollama is not available (extraction requires MedGemma).

**Flow:** Check extraction count → navigate to review → verify entities (Hemoglobin, Glucose, etc.) → click Confirm.

**Screenshots:** `03-review-screen.png`, `03-review-confirmed.png`

### 04 — Chat With Data

**What it tests:** Chat UI rendering, message input, and AI response (or graceful degradation).

**Flow:** Navigate to chat → verify input field → type "What were my latest lab results?" → send → wait for response.

- **With Ollama:** Waits up to 60s for streaming AI response.
- **Without Ollama:** Verifies message is queued and UI degrades gracefully.

**Screenshots:** `04-chat-screen.png`, `04-chat-question-typed.png`, `04-chat-after-send.png`, `04-chat-no-ai.png`

### 05 — Navigation

**What it tests:** All 5 main screens render without errors.

**Screens visited:** Home, Chat, Documents, Timeline, Settings.

**Per screen:** Navigate → wait 2s → screenshot → check for error text → verify expected content.

**Screenshots:** `05-screen-home.png`, `05-screen-chat.png`, `05-screen-documents.png`, `05-screen-timeline.png`, `05-screen-settings.png`

---

## Test Fixtures

Located in `e2e/fixtures/`. These are real PDFs (not mocks) because the import pipeline validates magic bytes, and vision OCR needs real page images.

### test-lab-results.pdf (3.8 KB)

**Patient:** Alice Martin, DOB 06/20/1985, Female
**Ordering Physician:** Dr. Robert Chen

| Panel | Tests |
|-------|-------|
| CBC | Hemoglobin 13.2, Hematocrit 39.8, WBC 7.2, Platelets 245, RBC 4.52, MCV 88.1 |
| Metabolic | Glucose 95, BUN 14, Creatinine 0.9, Na 140, K 4.1, Cl 102, CO2 24, Ca 9.4 |
| Lipid | Total Cholesterol 210 (borderline high), HDL 58, LDL 128, Triglycerides 120 |
| Thyroid | TSH 2.1, Free T4 1.2 |

### test-prescription.pdf (3.4 KB)

**Patient:** Alice Martin
**Prescriber:** Dr. Robert Chen, MD

| Medication | Dose | Frequency | Indication |
|------------|------|-----------|------------|
| Lisinopril | 10 mg | Once daily (morning) | Hypertension |
| Metformin | 500 mg | Twice daily with meals | Type 2 Diabetes |
| Atorvastatin | 20 mg | Once daily (bedtime) | Hyperlipidemia |

---

## Screenshots

### Output location

All screenshots are saved to `e2e/results/`. This directory is on the Windows filesystem at:
```
C:\Users\tkonlambigue\Antigravity\Coheara\e2e\results\
```

### Naming convention

| Pattern | Source |
|---------|--------|
| `{NN}-{step}.png` | Scenario step (e.g., `01-trust-screen.png`) |
| `fail-{NN}.png` | Diagnostic capture on scenario failure |
| `screen-{name}.png` | Ad-hoc screenshot via `screenshot.ts` |
| `timeout-{name}.png` | Diagnostic capture on wait timeout |
| `assert-fail-{type}.png` | Diagnostic capture on assertion failure |
| `current-view.png` | Default name for `screenshot.ts` without args |

### Reading screenshots (for AI operators)

```
Use the Read tool on any .png file path to view it visually.
Example: Read e2e/results/01-home-screen.png
```

### Report format

`e2e/results/report.json`:
```json
{
  "timestamp": "2026-03-02T22:33:00.000Z",
  "totalDurationMs": 74400,
  "ollamaAvailable": false,
  "scenarios": [
    {
      "id": "01",
      "name": "Create Profile",
      "status": "pass",
      "durationMs": 29600
    }
  ],
  "summary": { "pass": 5, "fail": 0, "skip": 0 }
}
```

---

## Writing New Scenarios

### Template

Create a new file `e2e/scenarios/06-my-test.ts`:

```typescript
import type { TestHarness } from '../harness.js';
import { CohearaDriver } from '../driver.js';

export async function run(harness: TestHarness): Promise<void> {
  const driver = new CohearaDriver(harness);

  // Navigate
  await driver.navigate('settings');
  await sleep(2000);
  await driver.screenshot('06-settings');

  // Interact
  await driver.clickByText('Language');
  await sleep(500);

  // Verify
  await driver.assertBodyContains('Settings');

  // IPC (bypass UI for data operations)
  const info = await driver.executeInvoke('get_active_profile_info');
  console.log('[06] Profile:', JSON.stringify(info));

  console.log('[06] PASS');
}

function sleep(ms: number): Promise<void> {
  return new Promise(r => setTimeout(r, ms));
}
```

### Register in suite runner

In `e2e/run-suite.ts`, add:

```typescript
import { run as scenario06 } from './scenarios/06-my-test.js';

// In the SCENARIOS array:
{ id: '06', name: 'My Test', fn: scenario06 },
```

### Form filling patterns

**Text input (Svelte-compatible):**
```typescript
await harness.browser!.execute(() => {
  const input = document.querySelector('input[type="text"]') as HTMLInputElement;
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!;
  setter.call(input, 'New Value');
  input.dispatchEvent(new Event('input', { bubbles: true }));
  input.dispatchEvent(new Event('change', { bubbles: true }));
});
```

**Pill-toggle button (e.g., sex selection):**
```typescript
await harness.browser!.execute(() => {
  const buttons = document.querySelectorAll('button');
  for (const btn of buttons) {
    if (btn.textContent?.trim() === 'Female') {
      btn.click();
      return;
    }
  }
});
```

**Chip tag (e.g., ethnicity selection):**
```typescript
await harness.browser!.execute(() => {
  const buttons = document.querySelectorAll('button');
  for (const btn of buttons) {
    if (btn.textContent?.trim() === 'European') {
      btn.click();
      return;
    }
  }
});
```

**Checkbox:**
```typescript
await harness.browser!.execute(() => {
  const cb = document.querySelector('input[type="checkbox"]') as HTMLInputElement;
  cb.click();
});
```

**Tauri IPC call:**
```typescript
const result = await driver.executeInvoke('list_profiles');
const queue = await driver.executeInvoke('get_import_queue');
await driver.executeInvoke('enqueue_imports', { paths: ['/path/to/file.pdf'] });
```

---

## Environment Variables

These are set automatically by the TestHarness on the tauri-driver process (which passes them to the app):

| Variable | Value | Purpose |
|----------|-------|---------|
| `GDK_BACKEND` | `x11` | Force X11 backend (not Wayland) |
| `LIBGL_ALWAYS_SOFTWARE` | `1` | Software OpenGL rendering |
| `WEBKIT_DISABLE_COMPOSITING_MODE` | `1` | Disable GPU compositing |
| `WEBKIT_DISABLE_DMABUF_RENDERER` | `1` | Disable DMA-BUF (critical for non-black screenshots) |
| `WEBKIT_HARDWARE_ACCELERATION_POLICY` | `never` | No GPU acceleration |
| `MESA_GL_VERSION_OVERRIDE` | `3.3` | Mesa compatibility |
| `DISPLAY` | `:99` | Xvfb virtual display |
| `XDG_DATA_HOME` | `e2e/.data` | Isolate WebKit storage from user data |

---

## Data Paths

| Path | What | Managed By |
|------|------|------------|
| `/tmp/coheara-build/x86_64-unknown-linux-gnu/debug/coheara` | Linux binary (136 MB) | `cargo build` |
| `~/Coheara-dev/profiles/` | Profile databases + encryption keys | App (debug builds) |
| `e2e/.data/com.coheara.app/` | WebKit cache, cookies, storage | WebKitGTK via XDG_DATA_HOME |
| `e2e/results/` | Screenshots + JSON report | TestHarness |
| `e2e/.harness.pid` | Running harness state (JSON) | TestHarness |
| `/tmp/.X99-lock` | Xvfb display lock | Xvfb |

**`resetApp()` clears:** `~/Coheara-dev/profiles/` and `e2e/.data/com.coheara.app/`.

---

## System Dependencies

Install once (already done if the suite runs):

```bash
# Virtual display
apt install -y xvfb

# Window detection for screenshots
apt install -y xdotool

# WebKitWebDriver binary (required by tauri-driver)
apt install -y webkit2gtk-driver

# ImageMagick for X11 screenshots
apt install -y imagemagick

# Mesa for software rendering
apt install -y mesa-utils libgl1-mesa-dri

# tauri-driver (WebDriver server for Tauri apps)
cargo install tauri-driver

# Node.js dependencies (from e2e/ directory)
cd e2e && npm install
```

### Verification

```bash
which Xvfb           # /usr/bin/Xvfb
which tauri-driver    # /root/.cargo/bin/tauri-driver
which xdotool         # /usr/bin/xdotool
which import          # /usr/bin/import (ImageMagick)
which WebKitWebDriver # /usr/bin/WebKitWebDriver
```

---

## Troubleshooting

### Black or empty screenshot

**Cause:** Missing `WEBKIT_DISABLE_DMABUF_RENDERER=1` environment variable.

**Fix:** The harness sets this automatically. If running manually, ensure all 6 env vars from the [Environment Variables](#environment-variables) table are set.

### "Cannot find binary WebKitWebDriver"

**Cause:** `webkit2gtk-driver` package not installed.

**Fix:** `apt install -y webkit2gtk-driver`

### Profile persists after resetApp()

**Cause:** Debug builds store data in `~/Coheara-dev/profiles/`, not in the `e2e/.data/` directory.

**Fix:** The harness clears both locations. If stale data persists:
```bash
rm -rf ~/Coheara-dev/profiles/
rm -rf e2e/.data/
```

### Cargo builds Windows binary instead of Linux

**Cause:** Default target is `x86_64-pc-windows-msvc` (cross-compilation setup).

**Fix:** The harness uses explicit `--target x86_64-unknown-linux-gnu --target-dir /tmp/coheara-build`. If building manually:
```bash
cargo build --manifest-path src-tauri/Cargo.toml \
  --target x86_64-unknown-linux-gnu \
  --target-dir /tmp/coheara-build
```

### tsx inline eval fails with module resolution

**Cause:** `npx tsx -e "import { ... } from './harness.js'"` can't resolve relative imports in eval context.

**Fix:** Always use script files, never inline `-e` evaluation.

### "Xvfb failed to start" / display occupied

**Cause:** Previous Xvfb still running or lock file orphaned.

**Fix:**
```bash
killall -9 Xvfb tauri-driver coheara WebKitWebDriver 2>/dev/null
rm -f /tmp/.X99-lock /tmp/.X98-lock
```

### Profile creation takes >30s

**Expected behavior.** Argon2 key derivation + AES-256-GCM encryption + SQLite DB creation are CPU-intensive. The scenario waits up to 60s.

### Scenario 03 always skips

**Expected** when Ollama is not running. Extraction review requires MedGemma for entity extraction. Start Ollama with a model to enable this scenario.

### WebDriver "Could not parse script result"

**Cause:** The Tauri IPC command returns a value that WebDriver can't serialize (e.g., complex Rust types).

**Fix:** Wrap the IPC call to return only JSON-serializable data, or catch the error and continue.

### Stale processes after crash

```bash
killall -9 Xvfb tauri-driver coheara WebKitWebDriver 2>/dev/null
rm -f /tmp/.X99-lock e2e/.harness.pid
```
