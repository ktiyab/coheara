<!--
  L6-03: AI Setup Wizard — Guided first-run setup for Ollama + model installation.

  5-step flow: Detect → Install → Pull → Verify → Done
  Full-screen (no tab bar). Accessible. Platform-aware.

  Entry points:
  - First unlock when no AI configured + not dismissed
  - Settings → "Set up AI" button
-->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    ollamaHealthCheck,
    listOllamaModels,
    getRecommendedModels,
    getActiveModel,
    setActiveModel,
    pullOllamaModel,
    cancelModelPull,
    setUserPreference,
    verifyAiModel,
    onPullProgress,
  } from '$lib/api/ai';
  import { ai } from '$lib/stores/ai.svelte';
  import { profile } from '$lib/stores/profile.svelte';
  import { navigation } from '$lib/stores/navigation.svelte';
  import type { RecommendedModel, ModelPullProgress } from '$lib/types/ai';
  import { isMedicalModel, formatModelSize } from '$lib/types/ai';

  // ── Wizard State ─────────────────────────────────────────

  type WizardStep = 'detect' | 'install' | 'pull' | 'verify' | 'done';

  let step = $state<WizardStep>('detect');
  let ollamaDetected = $state(false);
  let hasModels = $state(false);
  let verifyPassed = $state(false);
  let verifyRunning = $state(false);
  let error = $state<string | null>(null);
  let recommended = $state<RecommendedModel[]>([]);
  let pullInput = $state('');
  let detectedPlatform = $state<'windows' | 'macos' | 'linux'>('windows');
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let unlistenPull: (() => void) | null = null;

  // Step position for progress indicator
  const STEPS: WizardStep[] = ['detect', 'install', 'pull', 'verify'];
  let stepIndex = $derived(STEPS.indexOf(step));
  let stepCount = $derived(STEPS.length);

  // ── Lifecycle ────────────────────────────────────────────

  onMount(async () => {
    detectPlatform();
    unlistenPull = await onPullProgress(handlePullProgress);
    await runDetection();
  });

  onDestroy(() => {
    if (pollTimer) clearInterval(pollTimer);
    unlistenPull?.();
  });

  // ── Platform Detection ───────────────────────────────────

  function detectPlatform() {
    const ua = navigator.userAgent.toLowerCase();
    if (ua.includes('win')) {
      detectedPlatform = 'windows';
    } else if (ua.includes('mac')) {
      detectedPlatform = 'macos';
    } else {
      detectedPlatform = 'linux';
    }
  }

  // ── Step: Detect ─────────────────────────────────────────

  async function runDetection() {
    error = null;
    try {
      const health = await ollamaHealthCheck();
      ai.health = health;
      ollamaDetected = health.reachable;

      if (health.reachable) {
        // Check if models already installed
        const models = await listOllamaModels();
        ai.models = models;
        hasModels = models.length > 0;

        if (hasModels) {
          // Check if active model already set
          const active = await getActiveModel();
          ai.activeModel = active;
          if (active) {
            // Fully configured — skip to verify
            step = 'verify';
            await runVerify();
            return;
          }
          // Models exist but no active — skip to pull (for selection)
          recommended = await getRecommendedModels().catch(() => []);
          step = 'pull';
        } else {
          // Ollama running, no models — skip to pull
          recommended = await getRecommendedModels().catch(() => []);
          step = 'pull';
        }
      } else {
        // Ollama not found — go to install
        step = 'install';
        startPolling();
      }
    } catch {
      // Health check failed — go to install
      step = 'install';
      startPolling();
    }
  }

  // ── Step: Install (with polling) ─────────────────────────

  function startPolling() {
    if (pollTimer) clearInterval(pollTimer);
    pollTimer = setInterval(async () => {
      try {
        const health = await ollamaHealthCheck();
        if (health.reachable) {
          ollamaDetected = true;
          ai.health = health;
          if (pollTimer) {
            clearInterval(pollTimer);
            pollTimer = null;
          }
          // Auto-advance to pull
          const models = await listOllamaModels().catch(() => []);
          ai.models = models;
          hasModels = models.length > 0;
          recommended = await getRecommendedModels().catch(() => []);
          step = 'pull';
        }
      } catch {
        // Still not available — keep polling
      }
    }, 10_000);
  }

  async function retryDetection() {
    error = null;
    try {
      const health = await ollamaHealthCheck();
      ai.health = health;
      if (health.reachable) {
        ollamaDetected = true;
        if (pollTimer) {
          clearInterval(pollTimer);
          pollTimer = null;
        }
        const models = await listOllamaModels().catch(() => []);
        ai.models = models;
        hasModels = models.length > 0;
        recommended = await getRecommendedModels().catch(() => []);
        step = 'pull';
      }
    } catch {
      error = 'Ollama is still not detected. Make sure it is installed and running.';
    }
  }

  // ── Step: Pull ───────────────────────────────────────────

  function handlePullProgress(progress: ModelPullProgress) {
    ai.pullProgress = progress;
    if (progress.status === 'complete') {
      onPullComplete(progress.model_name);
    }
  }

  async function onPullComplete(modelName: string) {
    // Refresh model list and auto-select the pulled model
    try {
      ai.models = await listOllamaModels();
      ai.activeModel = await setActiveModel(modelName, 'wizard');
      hasModels = true;
      // Advance to verify
      step = 'verify';
      await runVerify();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function handlePull(name: string) {
    if (!name.trim()) return;
    error = null;
    try {
      ai.pullProgress = {
        status: 'starting',
        model_name: name,
        progress_percent: 0,
        bytes_completed: 0,
        bytes_total: 0,
        error_message: null,
      };
      await pullOllamaModel(name.trim());
      pullInput = '';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function handleCancelPull() {
    try {
      await cancelModelPull();
      ai.pullProgress = null;
    } catch {
      // Silent
    }
  }

  async function handleSelectExisting(name: string) {
    error = null;
    try {
      ai.activeModel = await setActiveModel(name, 'wizard');
      step = 'verify';
      await runVerify();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  // ── Step: Verify ─────────────────────────────────────────

  async function runVerify() {
    verifyRunning = true;
    verifyPassed = false;
    error = null;
    try {
      const modelName = ai.activeModel?.name;
      if (!modelName) {
        error = 'No model selected.';
        verifyRunning = false;
        return;
      }
      const ok = await verifyAiModel(modelName);
      verifyPassed = ok;
      if (ok) {
        step = 'done';
      } else {
        error = 'Model responded but verification failed. Try pulling the model again.';
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      verifyRunning = false;
    }
  }

  // ── Skip / Done ──────────────────────────────────────────

  async function handleSkip() {
    try {
      await setUserPreference('dismissed_ai_setup', 'true');
    } catch {
      // Best effort
    }
    navigation.navigate('home');
  }

  function handleDone() {
    // Update profile AI status
    profile.aiStatus = {
      ollama_available: true,
      ollama_model: ai.activeModel?.name ?? null,
      embedder_type: 'onnx',
      summary: 'AI engine ready',
    };
    navigation.navigate('home');
  }
</script>

<div class="flex flex-col min-h-screen bg-stone-50">
  <!-- Header with step indicator -->
  <header class="px-6 pt-6 pb-2">
    <h1 class="text-2xl font-bold text-stone-800 mb-4">Set Up AI Engine</h1>

    {#if step !== 'done'}
      <!-- Step progress indicator -->
      <div
        class="flex items-center gap-2 mb-2"
        aria-label="Step {stepIndex + 1} of {stepCount}: {step === 'detect' ? 'Checking AI Engine' : step === 'install' ? 'Install AI Engine' : step === 'pull' ? 'Download AI Model' : 'Verifying AI Engine'}"
        role="group"
      >
        {#each STEPS as s, i (s)}
          <div class="flex items-center gap-2">
            {#if i > 0}
              <div class="w-8 h-0.5 {i <= stepIndex ? 'bg-teal-500' : 'bg-stone-200'}"></div>
            {/if}
            <div
              class="w-8 h-8 rounded-full flex items-center justify-center text-sm font-medium
                     {i < stepIndex ? 'bg-teal-500 text-white' :
                      i === stepIndex ? 'bg-teal-600 text-white' :
                      'bg-stone-200 text-stone-500'}"
              aria-current={i === stepIndex ? 'step' : undefined}
            >
              {#if i < stepIndex}
                &#10003;
              {:else}
                {i + 1}
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </header>

  <!-- Step content -->
  <div class="flex-1 px-6 pb-6">
    {#if step === 'detect'}
      <!-- ═══ DETECT ═══ -->
      <div class="flex flex-col items-center justify-center py-16 text-center">
        <div class="animate-spin w-8 h-8 border-3 border-teal-500 border-t-transparent rounded-full mb-6" role="status" aria-label="Checking if Ollama is running"></div>
        <p class="text-lg text-stone-700 mb-2">Checking AI Engine...</p>
        <p class="text-sm text-stone-500">Looking for Ollama on your computer</p>
      </div>

    {:else if step === 'install'}
      <!-- ═══ INSTALL ═══ -->
      <div class="space-y-4">
        <div class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
          <p class="text-base text-stone-700 mb-4">
            Coheara uses <strong>Ollama</strong> to run AI models locally on your computer.
            Your data never leaves your machine.
          </p>

          {#if detectedPlatform === 'windows'}
            <div class="bg-blue-50 rounded-xl p-4 border border-blue-200">
              <h3 class="text-sm font-medium text-blue-800 mb-2">Windows</h3>
              <p class="text-sm text-blue-700 mb-3">
                Download Ollama from <span class="font-mono font-medium">ollama.com/download</span> and run the installer.
              </p>
              <p class="text-xs text-blue-600">After installing, start Ollama from your Start Menu.</p>
            </div>
          {:else if detectedPlatform === 'macos'}
            <div class="bg-blue-50 rounded-xl p-4 border border-blue-200">
              <h3 class="text-sm font-medium text-blue-800 mb-2">macOS</h3>
              <p class="text-sm text-blue-700 mb-3">
                Download from <span class="font-mono font-medium">ollama.com/download</span> or install with Homebrew:
              </p>
              <code class="block bg-blue-100 rounded-lg px-3 py-2 text-sm text-blue-900 select-all">brew install ollama</code>
              <p class="text-xs text-blue-600 mt-2">After installing, open Ollama from Applications.</p>
            </div>
          {:else}
            <div class="bg-blue-50 rounded-xl p-4 border border-blue-200">
              <h3 class="text-sm font-medium text-blue-800 mb-2">Linux</h3>
              <p class="text-sm text-blue-700 mb-3">Run this command in your terminal:</p>
              <code class="block bg-blue-100 rounded-lg px-3 py-2 text-sm text-blue-900 select-all">curl -fsSL https://ollama.com/install.sh | sh</code>
              <p class="text-xs text-blue-600 mt-2">After installing, start Ollama with: <code class="font-mono">ollama serve</code></p>
            </div>
          {/if}
        </div>

        {#if error}
          <div class="bg-red-50 rounded-xl p-4 border border-red-200">
            <p class="text-sm text-red-700">{error}</p>
          </div>
        {/if}

        <p class="text-sm text-stone-500 text-center">
          Waiting for Ollama... checking every 10 seconds.
        </p>

        <div class="flex gap-3">
          <button
            class="flex-1 px-4 py-3 bg-teal-600 text-white rounded-xl text-sm font-medium hover:bg-teal-700 min-h-[44px]"
            onclick={retryDetection}
          >
            Retry detection
          </button>
          <button
            class="px-4 py-3 border border-stone-200 rounded-xl text-sm text-stone-600 hover:bg-stone-50 min-h-[44px]"
            onclick={handleSkip}
          >
            Skip for now
          </button>
        </div>
      </div>

    {:else if step === 'pull'}
      <!-- ═══ PULL MODEL ═══ -->
      <div class="space-y-4">
        <p class="text-base text-stone-700">
          Choose a model for medical document analysis:
        </p>

        {#if error}
          <div class="bg-red-50 rounded-xl p-4 border border-red-200">
            <p class="text-sm text-red-700">{error}</p>
          </div>
        {/if}

        <!-- Pull progress (if active) -->
        {#if ai.isPulling && ai.pullProgress}
          <div class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
            <p class="text-sm font-medium text-stone-800 mb-2">Downloading {ai.pullProgress.model_name}...</p>
            <div
              class="w-full bg-stone-200 rounded-full h-2.5 mb-2"
              role="progressbar"
              aria-valuenow={Math.round(ai.pullProgress.progress_percent)}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-label="Downloading {ai.pullProgress.model_name}: {Math.round(ai.pullProgress.progress_percent)}%"
            >
              <div
                class="bg-teal-500 h-2.5 rounded-full transition-all"
                style="width: {ai.pullProgress.progress_percent}%"
              ></div>
            </div>
            <div class="flex justify-between items-center">
              <span class="text-xs text-stone-500">
                {formatModelSize(ai.pullProgress.bytes_completed)} / {formatModelSize(ai.pullProgress.bytes_total)}
                &middot; {Math.round(ai.pullProgress.progress_percent)}%
              </span>
              <button
                class="text-xs text-red-600 border border-red-200 px-3 py-1.5 rounded-lg hover:bg-red-50 min-h-[44px]"
                onclick={handleCancelPull}
              >
                Cancel
              </button>
            </div>
            <p class="text-xs text-stone-400 mt-2">First download may take several minutes depending on your connection.</p>
          </div>
        {/if}

        <!-- Pull error -->
        {#if ai.pullProgress?.status === 'error'}
          <div class="bg-red-50 rounded-xl p-4 border border-red-200">
            <p class="text-sm text-red-700">
              Failed: {ai.pullProgress.error_message ?? 'Unknown error'}
            </p>
          </div>
        {/if}

        <!-- Recommended models + custom input (when not pulling) -->
        {#if !ai.isPulling}
          <!-- Installed models (if user already had some) -->
          {#if ai.models.length > 0}
            <div class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
              <h3 class="text-sm font-medium text-stone-500 mb-3">ALREADY INSTALLED</h3>
              <div class="space-y-2">
                {#each ai.models as model (model.name)}
                  <div class="flex items-center gap-3 p-3 rounded-lg border border-stone-100">
                    <span aria-label={isMedicalModel(model.name) ? 'Medical model' : 'General model'}>
                      {isMedicalModel(model.name) ? '\u2605' : '\u25CB'}
                    </span>
                    <div class="flex-1 min-w-0">
                      <p class="text-sm font-medium text-stone-800 truncate">{model.name}</p>
                      <p class="text-xs text-stone-500">{formatModelSize(model.size)}</p>
                    </div>
                    <button
                      class="text-xs text-teal-700 border border-teal-200 px-3 py-1.5 rounded-lg hover:bg-teal-50 min-h-[44px]"
                      onclick={() => handleSelectExisting(model.name)}
                    >
                      Use this
                    </button>
                  </div>
                {/each}
              </div>
            </div>
          {/if}

          <!-- Recommended models to download -->
          {#if recommended.length > 0}
            <div class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
              <h3 class="text-sm font-medium text-stone-500 mb-3">RECOMMENDED MODELS</h3>
              <div class="space-y-3">
                {#each recommended as rec, i (rec.name)}
                  {@const alreadyInstalled = ai.models.some(m => m.name === rec.name)}
                  <div class="p-4 rounded-xl border {i === 0 ? 'border-teal-300 bg-teal-50' : 'border-stone-100'}">
                    <div class="flex items-start gap-3">
                      <span class="text-lg" aria-label="Medical model">{'\u2605'}</span>
                      <div class="flex-1">
                        <div class="flex items-center gap-2">
                          <p class="text-sm font-medium text-stone-800">{rec.name}</p>
                          {#if i === 0}
                            <span class="text-xs bg-teal-100 text-teal-700 px-2 py-0.5 rounded-full">Recommended</span>
                          {/if}
                        </div>
                        <p class="text-xs text-stone-600 mt-1">{rec.description}</p>
                        <p class="text-xs text-stone-500 mt-0.5">Requires {rec.min_ram_gb}GB+ RAM</p>
                      </div>
                      {#if alreadyInstalled}
                        <span class="text-xs text-stone-400 mt-1">Installed</span>
                      {:else}
                        <button
                          class="px-4 py-2 bg-teal-600 text-white text-sm rounded-lg hover:bg-teal-700 min-h-[44px]"
                          onclick={() => handlePull(rec.name)}
                        >
                          Download
                        </button>
                      {/if}
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          {/if}

          <!-- Custom model input -->
          <div class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
            <h3 class="text-sm font-medium text-stone-500 mb-3">OR ENTER MODEL NAME</h3>
            <div class="flex gap-2">
              <input
                type="text"
                bind:value={pullInput}
                placeholder="e.g., medgemma:4b"
                class="flex-1 text-sm border border-stone-200 rounded-lg px-3 py-2 text-stone-800 placeholder-stone-400 focus:outline-none focus:ring-2 focus:ring-teal-400 min-h-[44px]"
              />
              <button
                class="px-4 py-2 bg-teal-600 text-white text-sm rounded-lg hover:bg-teal-700 disabled:opacity-50 disabled:cursor-not-allowed min-h-[44px]"
                onclick={() => handlePull(pullInput)}
                disabled={!pullInput.trim()}
              >
                Pull
              </button>
            </div>
          </div>
        {/if}

        <div class="flex justify-end">
          <button
            class="px-4 py-3 border border-stone-200 rounded-xl text-sm text-stone-600 hover:bg-stone-50 min-h-[44px]"
            onclick={handleSkip}
          >
            Skip for now
          </button>
        </div>
      </div>

    {:else if step === 'verify'}
      <!-- ═══ VERIFY ═══ -->
      <div class="flex flex-col items-center justify-center py-16 text-center">
        {#if verifyRunning}
          <div class="animate-spin w-8 h-8 border-3 border-teal-500 border-t-transparent rounded-full mb-6" role="status" aria-label="Verifying AI model"></div>
          <p class="text-lg text-stone-700 mb-2">Testing {ai.activeModel?.name ?? 'AI model'}...</p>
          <p class="text-sm text-stone-500">First run may take longer as the model loads into memory.</p>
        {:else if error}
          <div class="text-4xl mb-4">&#9888;</div>
          <p class="text-lg text-stone-700 mb-2">Verification failed</p>
          <p class="text-sm text-red-600 mb-6">{error}</p>
          <button
            class="px-6 py-3 bg-teal-600 text-white rounded-xl text-sm font-medium hover:bg-teal-700 min-h-[44px]"
            onclick={runVerify}
          >
            Retry
          </button>
        {:else}
          <div class="text-4xl mb-4">&#10003;</div>
          <p class="text-lg text-stone-700 mb-2">AI model verified</p>
          <p class="text-sm text-stone-500">Continuing...</p>
        {/if}
      </div>

    {:else if step === 'done'}
      <!-- ═══ DONE ═══ -->
      <div class="flex flex-col items-center justify-center py-16 text-center">
        <div class="w-16 h-16 bg-teal-100 rounded-full flex items-center justify-center text-3xl text-teal-600 mb-6">
          &#10003;
        </div>
        <h2 class="text-xl font-semibold text-stone-800 mb-2">AI Engine Ready</h2>
        <p class="text-base text-stone-600 mb-2">
          {ai.activeModel?.name ?? 'Your AI model'} is set up and working.
        </p>
        <p class="text-sm text-stone-500 mb-8">
          You can change models anytime in Settings &rarr; AI Engine.
        </p>

        <div class="space-y-3 w-full max-w-xs text-left bg-white rounded-xl p-5 border border-stone-100 shadow-sm mb-8">
          <div class="flex items-center gap-2">
            <span class="text-teal-500">&#10003;</span>
            <span class="text-sm text-stone-700">Ollama running</span>
          </div>
          <div class="flex items-center gap-2">
            <span class="text-teal-500">&#10003;</span>
            <span class="text-sm text-stone-700">
              {ai.activeModel?.name}
              {#if ai.activeModel?.quality === 'Medical'}
                <span class="text-xs text-teal-600 ml-1">(Medical)</span>
              {/if}
            </span>
          </div>
          <div class="flex items-center gap-2">
            <span class="text-teal-500">&#10003;</span>
            <span class="text-sm text-stone-700">AI responding correctly</span>
          </div>
        </div>

        <button
          class="px-8 py-3 bg-teal-600 text-white rounded-xl text-base font-medium hover:bg-teal-700 min-h-[44px]"
          onclick={handleDone}
        >
          Go to Coheara
        </button>
      </div>
    {/if}
  </div>

  <!-- Graceful degradation info (shown on install and pull steps) -->
  {#if step === 'install' || step === 'pull'}
    <div class="px-6 pb-6">
      <div class="bg-stone-100 rounded-xl p-4">
        <p class="text-xs text-stone-500">
          <strong>Without AI:</strong> You can still import documents, track medications,
          and use the journal. AI features like document analysis and chat require Ollama.
        </p>
      </div>
    </div>
  {/if}
</div>
