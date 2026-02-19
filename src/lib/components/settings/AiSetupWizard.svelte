<!--
  L6-03: AI Setup Wizard — Guided first-run setup for Ollama + model installation.

  5-step flow: Detect → Install → Pull → Verify → Done
  Full-screen (no tab bar). Accessible. Platform-aware.

  Entry points:
  - First unlock when no AI configured + not dismissed
  - Settings → "Set up AI" button
-->
<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { t } from 'svelte-i18n';
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
  import Button from '$lib/components/ui/Button.svelte';

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
  let stepContentEl: HTMLDivElement | undefined = $state();

  // Link HTML constant for install instructions
  const ollamaLink = '<a href="https://ollama.com/download" target="_blank" rel="noopener noreferrer" class="font-mono font-medium underline hover:text-[var(--color-info-800)]">ollama.com/download</a>';
  const ollamaServeCmd = '<code class="font-mono">ollama serve</code>';

  // ACC-L6-15: Move focus to step content when step changes
  $effect(() => {
    // Subscribe to step (reactive read)
    const _currentStep = step;
    // After DOM update, focus the step container
    tick().then(() => {
      if (stepContentEl && _currentStep !== 'detect') {
        stepContentEl.focus();
      }
    });
  });

  // Step position for progress indicator
  const STEPS: WizardStep[] = ['detect', 'install', 'pull', 'verify'];
  let stepIndex = $derived(STEPS.indexOf(step));
  let stepCount = $derived(STEPS.length);

  // Derived step name for aria
  let currentStepName = $derived(
    step === 'detect' ? $t('ai.setup_step_detect') :
    step === 'install' ? $t('ai.setup_step_install') :
    step === 'pull' ? $t('ai.setup_step_pull') :
    $t('ai.setup_step_verify')
  );

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
      error = $t('ai.ollama_not_detected');
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
        error = $t('ai.no_model_selected');
        verifyRunning = false;
        return;
      }
      const ok = await verifyAiModel(modelName);
      verifyPassed = ok;
      if (ok) {
        step = 'done';
      } else {
        error = $t('ai.verify_failed');
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
    // S.5: Update both stores via unified applyStatus
    const status = {
      ollama_available: true,
      active_model: ai.activeModel ?? null,
      embedder_type: ai.embedderType || 'onnx',
      summary: $t('ai.engine_ready'),
      level: 'configured' as const,
    };
    ai.applyStatus(status);
    profile.aiStatus = status;
    navigation.navigate('home');
  }
</script>

<div class="flex flex-col min-h-screen bg-stone-50">
  <!-- Header with step indicator -->
  <header class="px-6 pt-6 pb-2">
    <h1 class="text-2xl font-bold text-stone-800 mb-4">{$t('ai.setup_heading')}</h1>

    {#if step !== 'done'}
      <!-- Step progress indicator -->
      <div
        class="flex items-center gap-2 mb-2"
        aria-label={$t('ai.step_progress', { values: { current: stepIndex + 1, total: stepCount, name: currentStepName } })}
        role="group"
      >
        {#each STEPS as s, i (s)}
          <div class="flex items-center gap-2">
            {#if i > 0}
              <div class="w-8 h-0.5 {i <= stepIndex ? 'bg-[var(--color-interactive)]' : 'bg-stone-200'}"></div>
            {/if}
            <div
              class="w-8 h-8 rounded-full flex items-center justify-center text-sm font-medium
                     {i < stepIndex ? 'bg-[var(--color-interactive)] text-white' :
                      i === stepIndex ? 'bg-[var(--color-interactive)] text-white' :
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

  <!-- ACC-L6-17: Step change announcement for screen readers -->
  <div role="status" aria-live="polite" class="sr-only" aria-atomic="true">
    {$t('ai.step_progress', { values: { current: stepIndex + 1, total: stepCount, name: currentStepName } })}
  </div>

  <!-- Step content (ACC-L6-15: focusable for screen reader announcements) -->
  <div class="flex-1 px-6 pb-6" bind:this={stepContentEl} tabindex="-1" style="outline: none;"
    aria-live="polite">
    {#if step === 'detect'}
      <!-- ═══ DETECT ═══ -->
      <div class="flex flex-col items-center justify-center py-16 text-center">
        <div class="animate-spin w-8 h-8 border-3 border-[var(--color-interactive)] border-t-transparent rounded-full mb-6" role="status" aria-label={$t('ai.checking_ollama')}></div>
        <p class="text-lg text-stone-700 mb-2">{$t('ai.checking_engine')}</p>
        <p class="text-sm text-stone-500">{$t('ai.looking_for_ollama')}</p>
      </div>

    {:else if step === 'install'}
      <!-- ═══ INSTALL ═══ -->
      <div class="space-y-4">
        <div class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
          <p class="text-base text-stone-700 mb-4">
            {@html $t('ai.ollama_local_description')}
          </p>

          {#if detectedPlatform === 'windows'}
            <div class="bg-[var(--color-info-50)] rounded-xl p-4 border border-[var(--color-info-200)]">
              <h2 class="text-sm font-medium text-[var(--color-info-800)] mb-2">{$t('ai.platform_windows')}</h2>
              <p class="text-sm text-[var(--color-info)] mb-3">
                {@html $t('ai.install_download', { values: { link: ollamaLink } })}
              </p>
              <p class="text-xs text-[var(--color-info)]">{$t('ai.install_windows_note')}</p>
            </div>
          {:else if detectedPlatform === 'macos'}
            <div class="bg-[var(--color-info-50)] rounded-xl p-4 border border-[var(--color-info-200)]">
              <h2 class="text-sm font-medium text-[var(--color-info-800)] mb-2">{$t('ai.platform_macos')}</h2>
              <p class="text-sm text-[var(--color-info)] mb-3">
                {@html $t('ai.install_macos_download', { values: { link: ollamaLink } })}
              </p>
              <code class="block bg-[var(--color-info-200)] rounded-lg px-3 py-2 text-sm text-[var(--color-info-800)] select-all">brew install ollama</code>
              <p class="text-xs text-[var(--color-info)] mt-2">{$t('ai.install_macos_note')}</p>
            </div>
          {:else}
            <div class="bg-[var(--color-info-50)] rounded-xl p-4 border border-[var(--color-info-200)]">
              <h2 class="text-sm font-medium text-[var(--color-info-800)] mb-2">{$t('ai.platform_linux')}</h2>
              <p class="text-sm text-[var(--color-info)] mb-3">{$t('ai.install_linux')}</p>
              <code class="block bg-[var(--color-info-200)] rounded-lg px-3 py-2 text-sm text-[var(--color-info-800)] select-all">curl -fsSL https://ollama.com/install.sh | sh</code>
              <p class="text-xs text-[var(--color-info)] mt-2">{@html $t('ai.install_linux_note', { values: { command: ollamaServeCmd } })}</p>
            </div>
          {/if}
        </div>

        {#if error}
          <div class="bg-[var(--color-danger-50)] rounded-xl p-4 border border-[var(--color-danger-200)]">
            <p class="text-sm text-[var(--color-danger)]">{error}</p>
          </div>
        {/if}

        <p class="text-sm text-stone-500 text-center">
          {$t('ai.waiting_for_ollama')}
        </p>

        <div class="flex gap-3">
          <Button variant="primary" onclick={retryDetection}>
            {$t('ai.retry_detection')}
          </Button>
          <Button variant="secondary" onclick={handleSkip}>
            {$t('ai.skip_for_now')}
          </Button>
        </div>
      </div>

    {:else if step === 'pull'}
      <!-- ═══ PULL MODEL ═══ -->
      <div class="space-y-4">
        <p class="text-base text-stone-700">
          {$t('ai.choose_model')}
        </p>

        {#if error}
          <div class="bg-[var(--color-danger-50)] rounded-xl p-4 border border-[var(--color-danger-200)]">
            <p class="text-sm text-[var(--color-danger)]">{error}</p>
          </div>
        {/if}

        <!-- Pull progress (if active) -->
        {#if ai.isPulling && ai.pullProgress}
          <div class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
            <p class="text-sm font-medium text-stone-800 mb-2">{$t('ai.downloading_model', { values: { name: ai.pullProgress.model_name } })}</p>
            <div
              class="w-full bg-stone-200 rounded-full h-2.5 mb-2"
              role="progressbar"
              aria-valuenow={Math.round(ai.pullProgress.progress_percent)}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-label={$t('ai.downloading_aria', { values: { name: ai.pullProgress.model_name, percent: Math.round(ai.pullProgress.progress_percent) } })}
            >
              <div
                class="bg-[var(--color-interactive)] h-2.5 rounded-full transition-all"
                style="width: {ai.pullProgress.progress_percent}%"
              ></div>
            </div>
            <div class="flex justify-between items-center">
              <span class="text-xs text-stone-500">
                {formatModelSize(ai.pullProgress.bytes_completed)} / {formatModelSize(ai.pullProgress.bytes_total)}
                &middot; {Math.round(ai.pullProgress.progress_percent)}%
              </span>
              <button
                class="text-xs text-[var(--color-danger)] border border-[var(--color-danger-200)] px-3 py-1.5 rounded-lg hover:bg-[var(--color-danger-50)] min-h-[44px]"
                onclick={handleCancelPull}
              >
                {$t('common.cancel')}
              </button>
            </div>
            <p class="text-xs text-stone-500 mt-2">{$t('ai.download_note')}</p>
          </div>
        {/if}

        <!-- Pull error -->
        {#if ai.pullProgress?.status === 'error'}
          <div class="bg-[var(--color-danger-50)] rounded-xl p-4 border border-[var(--color-danger-200)]">
            <p class="text-sm text-[var(--color-danger)]">
              {ai.pullProgress.error_message ?? $t('common.unknown')}
            </p>
          </div>
        {/if}

        <!-- Recommended models + custom input (when not pulling) -->
        {#if !ai.isPulling}
          <!-- Installed models (if user already had some) -->
          {#if ai.models.length > 0}
            <div class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
              <h2 class="text-sm font-medium text-stone-500 mb-3">{$t('ai.already_installed')}</h2>
              <div class="space-y-2">
                {#each ai.models as model (model.name)}
                  <div class="flex items-center gap-3 p-3 rounded-lg border border-stone-100">
                    <span aria-label={isMedicalModel(model.name) ? $t('ai.medical_model') : $t('ai.general_model')}>
                      {isMedicalModel(model.name) ? '\u2605' : '\u25CB'}
                    </span>
                    <div class="flex-1 min-w-0">
                      <p class="text-sm font-medium text-stone-800 truncate">{model.name}</p>
                      <p class="text-xs text-stone-500">{formatModelSize(model.size)}</p>
                    </div>
                    <button
                      class="text-xs text-[var(--color-interactive)] border border-[var(--color-interactive)] px-3 py-1.5 rounded-lg hover:bg-[var(--color-interactive-50)] min-h-[44px]"
                      onclick={() => handleSelectExisting(model.name)}
                    >
                      {$t('ai.use_this')}
                    </button>
                  </div>
                {/each}
              </div>
            </div>
          {/if}

          <!-- Recommended models to download -->
          {#if recommended.length > 0}
            <div class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
              <h2 class="text-sm font-medium text-stone-500 mb-3">{$t('ai.recommended_models')}</h2>
              <div class="space-y-3">
                {#each recommended as rec, i (rec.name)}
                  {@const alreadyInstalled = ai.models.some(m => m.name === rec.name)}
                  <div class="p-4 rounded-xl border {i === 0 ? 'border-[var(--color-interactive)] bg-[var(--color-interactive-50)]' : 'border-stone-100'}">
                    <div class="flex items-start gap-3">
                      <span class="text-lg" aria-label={$t('ai.medical_model')}>{'\u2605'}</span>
                      <div class="flex-1">
                        <div class="flex items-center gap-2">
                          <p class="text-sm font-medium text-stone-800">{rec.name}</p>
                          {#if i === 0}
                            <span class="text-xs bg-[var(--color-interactive-50)] text-[var(--color-interactive)] px-2 py-0.5 rounded-full">{$t('ai.recommended_badge')}</span>
                          {/if}
                        </div>
                        <p class="text-xs text-stone-600 mt-1">{rec.description}</p>
                        <p class="text-xs text-stone-500 mt-0.5">{$t('ai.requires_ram', { values: { gb: rec.min_ram_gb } })}</p>
                      </div>
                      {#if alreadyInstalled}
                        <span class="text-xs text-stone-500 mt-1">{$t('ai.installed_tag')}</span>
                      {:else}
                        <button
                          class="px-4 py-2 bg-[var(--color-interactive)] text-white text-sm rounded-lg hover:bg-[var(--color-interactive-hover)] min-h-[44px]"
                          onclick={() => handlePull(rec.name)}
                        >
                          {$t('ai.download_button')}
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
            <h2 class="text-sm font-medium text-stone-500 mb-3">{$t('ai.custom_model_heading')}</h2>
            <div class="flex gap-2">
              <input
                type="text"
                bind:value={pullInput}
                placeholder={$t('ai.model_placeholder')}
                class="flex-1 text-sm border border-stone-200 rounded-lg px-3 py-2 text-stone-800 placeholder-stone-400 focus:outline-none focus:ring-2 focus:ring-[var(--color-interactive)] min-h-[44px]"
              />
              <button
                class="px-4 py-2 bg-[var(--color-interactive)] text-white text-sm rounded-lg hover:bg-[var(--color-interactive-hover)] disabled:opacity-50 disabled:cursor-not-allowed min-h-[44px]"
                onclick={() => handlePull(pullInput)}
                disabled={!pullInput.trim()}
              >
                {$t('ai.pull')}
              </button>
            </div>
          </div>
        {/if}

        <div class="flex justify-end">
          <Button variant="secondary" onclick={handleSkip}>
            {$t('ai.skip_for_now')}
          </Button>
        </div>
      </div>

    {:else if step === 'verify'}
      <!-- ═══ VERIFY ═══ -->
      <div class="flex flex-col items-center justify-center py-16 text-center">
        {#if verifyRunning}
          <div class="animate-spin w-8 h-8 border-3 border-[var(--color-interactive)] border-t-transparent rounded-full mb-6" role="status" aria-label={$t('ai.verifying_model')}></div>
          <p class="text-lg text-stone-700 mb-2">{$t('ai.testing_model', { values: { name: ai.activeModel?.name ?? 'AI' } })}</p>
          <p class="text-sm text-stone-500">{$t('ai.first_run_note')}</p>
        {:else if error}
          <div class="text-4xl mb-4">&#9888;</div>
          <p class="text-lg text-stone-700 mb-2">{$t('ai.verification_failed')}</p>
          <p class="text-sm text-[var(--color-danger)] mb-6">{error}</p>
          <Button variant="primary" onclick={runVerify}>
            {$t('common.retry')}
          </Button>
        {:else}
          <div class="text-4xl mb-4">&#10003;</div>
          <p class="text-lg text-stone-700 mb-2">{$t('ai.model_verified')}</p>
          <p class="text-sm text-stone-500">{$t('ai.continuing')}</p>
        {/if}
      </div>

    {:else if step === 'done'}
      <!-- ═══ DONE ═══ -->
      <div class="flex flex-col items-center justify-center py-16 text-center">
        <div class="w-16 h-16 bg-[var(--color-interactive-50)] rounded-full flex items-center justify-center text-3xl text-[var(--color-interactive)] mb-6">
          &#10003;
        </div>
        <h2 class="text-xl font-semibold text-stone-800 mb-2">{$t('ai.engine_ready_heading')}</h2>
        <p class="text-base text-stone-600 mb-2">
          {$t('ai.model_setup_complete', { values: { name: ai.activeModel?.name ?? 'AI' } })}
        </p>
        <p class="text-sm text-stone-500 mb-8">
          {$t('ai.change_model_hint')}
        </p>

        <div class="space-y-3 w-full max-w-xs text-left bg-white rounded-xl p-5 border border-stone-100 shadow-sm mb-8">
          <div class="flex items-center gap-2">
            <span class="text-[var(--color-interactive)]">&#10003;</span>
            <span class="text-sm text-stone-700">{$t('ai.ollama_running')}</span>
          </div>
          <div class="flex items-center gap-2">
            <span class="text-[var(--color-interactive)]">&#10003;</span>
            <span class="text-sm text-stone-700">
              {ai.activeModel?.name}
              {#if ai.activeModel?.quality === 'Medical'}
                <span class="text-xs text-[var(--color-interactive)] ml-1">{$t('ai.medical_quality')}</span>
              {/if}
            </span>
          </div>
          <div class="flex items-center gap-2">
            <span class="text-[var(--color-interactive)]">&#10003;</span>
            <span class="text-sm text-stone-700">{$t('ai.ai_responding')}</span>
          </div>
        </div>

        <Button variant="primary" size="lg" onclick={handleDone}>
          {$t('ai.go_to_app')}
        </Button>
      </div>
    {/if}
  </div>

  <!-- Graceful degradation info (shown on install and pull steps) -->
  {#if step === 'install' || step === 'pull'}
    <div class="px-6 pb-6">
      <div class="bg-stone-100 rounded-xl p-4">
        <p class="text-xs text-stone-500">
          {@html $t('ai.without_ai')}
        </p>
      </div>
    </div>
  {/if}
</div>
