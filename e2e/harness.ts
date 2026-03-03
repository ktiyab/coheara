/**
 * E2E-01 Brick 2: TestHarness — Full lifecycle management for headless Coheara testing.
 *
 * Architecture (proven via manual testing):
 *   1. Xvfb provides virtual X11 display
 *   2. tauri-driver starts with headless env vars, manages app lifecycle
 *   3. WebDriver session → tauri-driver launches app via capabilities
 *   4. WebDriver handles DOM interaction (click, type, executeScript)
 *   5. ImageMagick `import` captures X11 window for screenshots
 *      (WebDriver screenshots fail with WebKitGTK compositing disabled)
 *
 * Required env for headless WebKitGTK:
 *   LIBGL_ALWAYS_SOFTWARE=1, WEBKIT_DISABLE_COMPOSITING_MODE=1,
 *   WEBKIT_DISABLE_DMABUF_RENDERER=1, GDK_BACKEND=x11
 */

import { spawn, execSync, type ChildProcess } from 'node:child_process';
import { existsSync, mkdirSync, writeFileSync, readFileSync, unlinkSync, statSync, readdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { remote, type Browser } from 'webdriverio';

// ── Constants ────────────────────────────────────────────────────────────────

const PROJECT_ROOT = resolve(import.meta.dirname, '..');
const TAURI_DIR = join(PROJECT_ROOT, 'src-tauri');
const BUILD_TARGET_DIR = '/tmp/coheara-build';
const LINUX_TARGET = 'x86_64-unknown-linux-gnu';
const BINARY_PATH = join(BUILD_TARGET_DIR, LINUX_TARGET, 'debug', 'coheara');
const RESULTS_DIR = join(PROJECT_ROOT, 'e2e', 'results');
const PID_FILE = join(PROJECT_ROOT, 'e2e', '.harness.pid');
const CARGO = '/root/.cargo/bin/cargo';

/** Environment variables required for headless WebKitGTK rendering in Xvfb. */
const HEADLESS_ENV: Record<string, string> = {
  GDK_BACKEND: 'x11',
  LIBGL_ALWAYS_SOFTWARE: '1',
  WEBKIT_DISABLE_COMPOSITING_MODE: '1',
  WEBKIT_DISABLE_DMABUF_RENDERER: '1',
  WEBKIT_HARDWARE_ACCELERATION_POLICY: 'never',
  MESA_GL_VERSION_OVERRIDE: '3.3',
};

const DEFAULT_DISPLAY = 99;
const DEFAULT_DRIVER_PORT = 4444;
const XVFB_RESOLUTION = '1280x800x24';
const DRIVER_READY_TIMEOUT_MS = 15_000;
const BUILD_TIMEOUT_MS = 600_000;
const PAGE_LOAD_WAIT_MS = 8_000;

// ── Helpers ──────────────────────────────────────────────────────────────────

function sleep(ms: number): Promise<void> {
  return new Promise(r => setTimeout(r, ms));
}

function isPortOpen(port: number): boolean {
  try {
    execSync(`curl -s -o /dev/null -w '%{http_code}' http://localhost:${port}/status`, {
      timeout: 3000,
      stdio: 'pipe',
    });
    return true;
  } catch {
    return false;
  }
}

function isDisplayFree(display: number): boolean {
  return !existsSync(`/tmp/.X${display}-lock`);
}

function findNewestMtime(dir: string, ext: string): number {
  let newest = 0;
  try {
    const entries = readdirSync(dir, { withFileTypes: true, recursive: true });
    for (const entry of entries) {
      if (entry.isFile() && entry.name.endsWith(ext)) {
        const fullPath = join(entry.parentPath ?? dir, entry.name);
        const mtime = statSync(fullPath).mtimeMs;
        if (mtime > newest) newest = mtime;
      }
    }
  } catch { /* directory may not exist */ }
  return newest;
}

function killProcess(proc: ChildProcess | null, label: string): void {
  if (!proc || proc.killed || proc.exitCode !== null) return;
  try {
    proc.kill('SIGTERM');
    setTimeout(() => {
      if (!proc.killed && proc.exitCode === null) {
        proc.kill('SIGKILL');
        console.log(`[harness] ${label}: SIGKILL after SIGTERM timeout`);
      }
    }, 5000);
  } catch {
    // Process already dead
  }
}

// ── TestHarness ──────────────────────────────────────────────────────────────

export class TestHarness {
  display: number;
  driverPort: number;
  browser: Browser | null = null;
  hasOllama = false;
  isReady = false;

  private xvfbProcess: ChildProcess | null = null;
  private driverProcess: ChildProcess | null = null;

  constructor(display = DEFAULT_DISPLAY, driverPort = DEFAULT_DRIVER_PORT) {
    this.display = display;
    this.driverPort = driverPort;
  }

  // ── Public API ─────────────────────────────────────────────────────────

  async setup(): Promise<void> {
    console.log('[harness] Starting E2E test harness...');
    mkdirSync(RESULTS_DIR, { recursive: true });

    this.display = this.findFreeDisplay();
    await this.startXvfb();
    await this.buildIfNeeded();
    await this.startDriver();
    await this.connectBrowser();
    // Wait for SvelteKit to hydrate and render
    await sleep(PAGE_LOAD_WAIT_MS);
    await this.detectOllama();

    this.isReady = true;
    this.writePidFile();
    console.log(`[harness] Ready. Display=:${this.display}, Port=${this.driverPort}, Ollama=${this.hasOllama}`);
  }

  async teardown(): Promise<void> {
    console.log('[harness] Tearing down...');
    this.isReady = false;

    if (this.browser) {
      try { await this.browser.deleteSession(); } catch { /* session may already be gone */ }
      this.browser = null;
    }

    killProcess(this.driverProcess, 'tauri-driver');
    killProcess(this.xvfbProcess, 'Xvfb');

    this.driverProcess = null;
    this.xvfbProcess = null;

    // Kill any orphan app/driver processes
    try { execSync('killall -9 coheara WebKitWebDriver 2>/dev/null', { stdio: 'pipe' }); } catch { /* ok */ }

    // Clean up orphaned lock file
    const lockFile = `/tmp/.X${this.display}-lock`;
    if (existsSync(lockFile)) {
      try { unlinkSync(lockFile); } catch { /* ignore */ }
    }

    this.removePidFile();
    console.log('[harness] Teardown complete.');
  }

  /**
   * Take a screenshot using ImageMagick `import` (X11 window capture).
   * WebDriver `takeScreenshot()` fails with WebKitGTK compositing disabled,
   * so we capture directly from the X11 display.
   */
  async screenshot(name: string): Promise<string> {
    const filePath = join(RESULTS_DIR, `${name}.png`);

    // Find the app window
    const wid = this.findWindowId();
    if (!wid) {
      throw new Error('No window found for screenshot');
    }

    try {
      execSync(
        `DISPLAY=:${this.display} import -window ${wid} "${filePath}"`,
        { stdio: 'pipe', timeout: 10_000 },
      );
    } catch (e) {
      throw new Error(`Screenshot failed: ${e}`);
    }

    console.log(`[screenshot] ${filePath}`);
    return filePath;
  }

  async resetApp(): Promise<void> {
    console.log('[harness] Resetting app (fresh DB)...');

    // Delete browser session (kills the app via tauri-driver)
    if (this.browser) {
      try { await this.browser.deleteSession(); } catch { /* ignore */ }
      this.browser = null;
    }

    // Remove app data — debug builds use ~/Coheara-dev/ (config.rs DAT-01)
    const homeDir = process.env.HOME ?? '/root';
    const appDataDir = join(homeDir, 'Coheara-dev');
    try { execSync(`rm -rf "${appDataDir}/profiles"`, { stdio: 'pipe' }); } catch { /* may not exist */ }
    // Also clear WebKit data stored via XDG_DATA_HOME
    const webkitDataDir = join(PROJECT_ROOT, 'e2e', '.data', 'com.coheara.app');
    try { execSync(`rm -rf "${webkitDataDir}"`, { stdio: 'pipe' }); } catch { /* may not exist */ }

    // Reconnect (tauri-driver launches fresh app instance)
    await this.connectBrowser();
    await sleep(PAGE_LOAD_WAIT_MS);
    console.log('[harness] App reset complete.');
  }

  isOllamaAvailable(): boolean {
    return this.hasOllama;
  }

  // ── Private ────────────────────────────────────────────────────────────

  private findFreeDisplay(): number {
    for (let d = DEFAULT_DISPLAY; d >= 90; d--) {
      if (isDisplayFree(d)) return d;
    }
    throw new Error('No free X display found (tried :99 through :90)');
  }

  private findWindowId(): string | null {
    try {
      const result = execSync(
        `DISPLAY=:${this.display} xdotool search --name "" 2>/dev/null`,
        { stdio: 'pipe', timeout: 3000 },
      ).toString().trim();
      // Return first window ID
      return result.split('\n')[0] || null;
    } catch {
      return null;
    }
  }

  private async startXvfb(): Promise<void> {
    console.log(`[harness] Starting Xvfb on :${this.display}...`);

    this.xvfbProcess = spawn('Xvfb', [
      `:${this.display}`,
      '-screen', '0', XVFB_RESOLUTION,
      '-ac',
      '-nolisten', 'tcp',
    ], {
      stdio: ['ignore', 'pipe', 'pipe'],
    });

    this.xvfbProcess.stderr?.on('data', (d: Buffer) => {
      const msg = d.toString().trim();
      if (msg && !msg.includes('Could not resolve keysym')) {
        console.log(`[Xvfb] ${msg}`);
      }
    });

    // Wait for lock file to appear
    const deadline = Date.now() + 10_000;
    while (Date.now() < deadline) {
      if (existsSync(`/tmp/.X${this.display}-lock`)) {
        console.log(`[harness] Xvfb running on :${this.display}`);
        return;
      }
      await sleep(200);
    }
    throw new Error(`Xvfb failed to start on :${this.display}`);
  }

  private async buildIfNeeded(): Promise<void> {
    const frontendSrcDir = join(PROJECT_ROOT, 'src');
    const buildDir = join(PROJECT_ROOT, 'build');

    const binaryExists = existsSync(BINARY_PATH);
    if (binaryExists) {
      const binaryMtime = statSync(BINARY_PATH).mtimeMs;
      // Check both Rust backend AND frontend sources — Tauri embeds frontend at build time
      const rustMtime = findNewestMtime(join(TAURI_DIR, 'src'), '.rs');
      const svelteMtime = findNewestMtime(frontendSrcDir, '.svelte');
      const tsMtime = findNewestMtime(frontendSrcDir, '.ts');
      const jsonMtime = findNewestMtime(join(frontendSrcDir, 'lib', 'i18n'), '.json');
      const srcMtime = Math.max(rustMtime, svelteMtime, tsMtime, jsonMtime);
      if (srcMtime <= binaryMtime) {
        console.log('[harness] Binary is up-to-date, skipping build.');
        return;
      }
      const staleSource = srcMtime === rustMtime ? 'Rust' :
                          srcMtime === svelteMtime ? 'Svelte' :
                          srcMtime === tsMtime ? 'TypeScript' : 'i18n JSON';
      console.log(`[harness] Binary stale (newest ${staleSource} source newer than binary).`);
    }

    // Step 1: Build frontend if needed (Tauri embeds ../build/ at compile time)
    // Plain `cargo build` does NOT trigger tauri.conf.json's beforeBuildCommand,
    // so we must run the frontend build explicitly.
    const frontendNewest = Math.max(
      findNewestMtime(frontendSrcDir, '.svelte'),
      findNewestMtime(frontendSrcDir, '.ts'),
      findNewestMtime(join(frontendSrcDir, 'lib', 'i18n'), '.json'),
    );
    const buildMtime = existsSync(buildDir) ? findNewestMtime(buildDir, '.js') : 0;
    if (frontendNewest > buildMtime || buildMtime === 0) {
      console.log('[harness] Building frontend (npm run build)...');
      try {
        execSync('npm run build', {
          cwd: PROJECT_ROOT,
          stdio: 'inherit',
          timeout: 360_000,
          env: { ...process.env },
        });
        console.log('[harness] Frontend build complete.');
      } catch (e) {
        throw new Error(`Frontend build failed: ${e}`);
      }
    } else {
      console.log('[harness] Frontend build/ is up-to-date, skipping.');
    }

    // Step 2: Build Rust binary (embeds the freshly built frontend from ../build/)
    console.log('[harness] Building Linux binary... (this may take a few minutes)');
    try {
      execSync(
        `${CARGO} build --manifest-path ${join(TAURI_DIR, 'Cargo.toml')} --target ${LINUX_TARGET} --target-dir ${BUILD_TARGET_DIR}`,
        {
          stdio: 'inherit',
          timeout: BUILD_TIMEOUT_MS,
          env: { ...process.env, DISPLAY: `:${this.display}` },
        },
      );
      console.log('[harness] Build complete.');
    } catch (e) {
      throw new Error(`Cargo build failed: ${e}`);
    }
  }

  private async startDriver(): Promise<void> {
    console.log(`[harness] Starting tauri-driver on port ${this.driverPort}...`);

    this.driverProcess = spawn('tauri-driver', ['--port', String(this.driverPort)], {
      env: {
        ...process.env,
        ...HEADLESS_ENV,
        DISPLAY: `:${this.display}`,
        XDG_DATA_HOME: join(PROJECT_ROOT, 'e2e', '.data'),
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    });

    this.driverProcess.stderr?.on('data', (d: Buffer) => {
      const msg = d.toString().trim();
      if (msg) console.log(`[tauri-driver] ${msg}`);
    });

    // Wait for driver to respond on /status
    const deadline = Date.now() + DRIVER_READY_TIMEOUT_MS;
    while (Date.now() < deadline) {
      if (isPortOpen(this.driverPort)) {
        console.log(`[harness] tauri-driver ready on port ${this.driverPort}`);
        return;
      }
      await sleep(500);
    }
    throw new Error(`tauri-driver did not become ready within ${DRIVER_READY_TIMEOUT_MS / 1000}s`);
  }

  /**
   * Create WebDriver session. tauri-driver launches the app binary
   * via the `tauri:options.application` capability.
   */
  private async connectBrowser(): Promise<void> {
    console.log('[harness] Connecting WebDriver (tauri-driver launches app)...');

    this.browser = await remote({
      hostname: 'localhost',
      port: this.driverPort,
      // Increase timeouts for CPU-bound Ollama inference:
      // executeScript calls can hang for minutes when Ollama pins CPU
      connectionRetryTimeout: 180_000,
      connectionRetryCount: 3,
      capabilities: {
        'tauri:options': {
          application: BINARY_PATH,
        },
        // W3C WebDriver session timeouts (ms)
        timeouts: {
          script: 300_000,     // executeScript timeout: 5 min (OCR calls can block IPC)
          pageLoad: 60_000,    // page load: 60s
          implicit: 10_000,    // implicit wait: 10s
        },
      } as WebdriverIO.Capabilities,
    });

    console.log('[harness] WebDriver session created.');
  }

  private async detectOllama(): Promise<void> {
    // Try host-side detection first (more reliable than webview fetch)
    try {
      execSync('curl -sf http://localhost:11434/api/version', { timeout: 5000, stdio: 'pipe' });
      this.hasOllama = true;
    } catch {
      // Fallback: try via webview
      try {
        const result = await this.browser!.execute(() => {
          return fetch('http://localhost:11434/api/version')
            .then(r => r.ok)
            .catch(() => false);
        });
        this.hasOllama = !!result;
      } catch {
        this.hasOllama = false;
      }
    }
    console.log(`[harness] Ollama available: ${this.hasOllama}`);
  }

  private writePidFile(): void {
    writeFileSync(PID_FILE, JSON.stringify({
      pid: process.pid,
      display: this.display,
      driverPort: this.driverPort,
      startedAt: new Date().toISOString(),
    }));
  }

  private removePidFile(): void {
    try { unlinkSync(PID_FILE); } catch { /* ignore */ }
  }

  // ── Static helpers ─────────────────────────────────────────────────────

  static isRunning(): { running: boolean; display?: number; driverPort?: number } {
    if (!existsSync(PID_FILE)) return { running: false };
    try {
      const data = JSON.parse(readFileSync(PID_FILE, 'utf-8'));
      process.kill(data.pid, 0);
      return { running: true, display: data.display, driverPort: data.driverPort };
    } catch {
      try { unlinkSync(PID_FILE); } catch { /* ignore */ }
      return { running: false };
    }
  }
}
