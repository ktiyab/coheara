<!--
  L6-02: AI Settings Screen — Model management interface.

  Provides: installed model list, model selection, pull interface,
  delete with confirmation, and Ollama health status.

  Entry points:
  - Settings tab → "AI Engine"
  - Amber AI status banner click
  - L6-03 Setup Wizard redirect
-->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { t } from 'svelte-i18n';
  import {
    ollamaHealthCheck,
    listOllamaModels,
    getRecommendedModels,
    getActiveModel,
    setActiveModel,
    deleteOllamaModel,
    pullOllamaModel,
    cancelModelPull,
    onPullProgress,
  } from '$lib/api/ai';
  import { ai } from '$lib/stores/ai.svelte';
  import { navigation } from '$lib/stores/navigation.svelte';
  import type { ModelInfo, RecommendedModel, ModelPullProgress } from '$lib/types/ai';
  import { isMedicalModel, formatModelSize, sourceDisplayText } from '$lib/types/ai';
  import BackButton from '$lib/components/ui/BackButton.svelte';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import ErrorState from '$lib/components/ui/ErrorState.svelte';
  import Button from '$lib/components/ui/Button.svelte';

  let recommended = $state<RecommendedModel[]>([]);
  let pullInput = $state('');
  let deleteConfirm = $state<string | null>(null);
  let nonMedicalWarning = $state<string | null>(null);
  let unlistenPull: (() => void) | null = null;

  onMount(async () => {
    ai.loading = true;
    ai.error = null;

    try {
      // Load all data in parallel
      const [health, models, active, recs] = await Promise.all([
        ollamaHealthCheck().catch(() => null),
        listOllamaModels().catch(() => [] as ModelInfo[]),
        getActiveModel().catch(() => null),
        getRecommendedModels().catch(() => [] as RecommendedModel[]),
      ]);

      ai.health = health;
      ai.models = models;
      ai.activeModel = active;
      recommended = recs;
    } catch (e) {
      ai.error = e instanceof Error ? e.message : String(e);
    } finally {
      ai.loading = false;
    }

    // Listen for pull progress events
    unlistenPull = await onPullProgress(handlePullProgress);
  });

  onDestroy(() => {
    unlistenPull?.();
  });

  function handlePullProgress(progress: ModelPullProgress) {
    ai.pullProgress = progress;
    if (progress.status === 'complete') {
      // Refresh model list and auto-select if no active model (AC-24)
      onPullComplete(progress.model_name);
    }
  }

  async function onPullComplete(pulledName: string) {
    try {
      const [models, active] = await Promise.all([
        listOllamaModels(),
        getActiveModel(),
      ]);
      ai.models = models;
      ai.activeModel = active;

      // AC-24: Auto-select the pulled model when no active model exists
      if (!active) {
        ai.activeModel = await setActiveModel(pulledName);
      }
    } catch {
      // Silent — don't overwrite existing state
    }
  }

  async function refreshModels() {
    try {
      const [models, active] = await Promise.all([
        listOllamaModels(),
        getActiveModel(),
      ]);
      ai.models = models;
      ai.activeModel = active;
    } catch {
      // Silent — don't overwrite existing state
    }
  }

  async function handleSelectModel(name: string) {
    if (!isMedicalModel(name)) {
      nonMedicalWarning = name;
      return;
    }
    await doSetModel(name);
  }

  async function doSetModel(name: string) {
    try {
      ai.activeModel = await setActiveModel(name);
      nonMedicalWarning = null;
    } catch (e) {
      ai.error = e instanceof Error ? e.message : String(e);
    }
  }

  async function handleDelete(name: string) {
    try {
      await deleteOllamaModel(name);
      deleteConfirm = null;
      await refreshModels();
    } catch (e) {
      ai.error = e instanceof Error ? e.message : String(e);
    }
  }

  async function handlePull(name: string) {
    if (!name.trim()) return;
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
      ai.error = e instanceof Error ? e.message : String(e);
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

  async function handleRetry() {
    ai.error = null;
    ai.loading = true;
    try {
      ai.health = await ollamaHealthCheck();
      ai.models = await listOllamaModels();
      ai.activeModel = await getActiveModel();
    } catch (e) {
      ai.error = e instanceof Error ? e.message : String(e);
    } finally {
      ai.loading = false;
    }
  }
</script>

<div class="flex flex-col min-h-screen pb-20 bg-stone-50">
  <!-- Header -->
  <header class="px-6 pt-6 pb-4 flex items-center gap-3">
    <BackButton />
    <h1 class="text-2xl font-bold text-stone-800">{$t('ai.settings_heading')}</h1>
  </header>

  {#if ai.loading}
    <LoadingState message={$t('ai.loading_settings')} />

  {:else if ai.error}
    <ErrorState
      message={ai.error}
      onretry={handleRetry}
      retryLabel={$t('common.retry')}
    />

  {:else if !ai.isOllamaReachable}
    <!-- Ollama not running -->
    <div class="px-6 space-y-4">
      <div class="bg-[var(--color-warning-50)] rounded-xl p-5 border border-[var(--color-warning-200)]">
        <h2 class="text-base font-medium text-[var(--color-warning-800)] mb-2">{$t('ai.ollama_not_running')}</h2>
        <p class="text-sm text-[var(--color-warning-800)] mb-4">
          {$t('ai.ollama_needed')}
        </p>
        <div class="space-y-2 text-sm text-stone-600">
          <p><strong>{$t('ai.install_label')}</strong> {$t('ai.install_visit', { values: { url: 'ollama.com/download' } })}</p>
          <p><strong>{$t('ai.start_label')}</strong> {$t('ai.start_ollama')}</p>
        </div>
        <button
          class="mt-4 px-4 py-2 bg-[var(--color-warning-200)] border border-[var(--color-warning-200)] rounded-lg text-sm text-[var(--color-warning-800)] hover:bg-[var(--color-warning-200)] min-h-[44px]"
          onclick={handleRetry}
        >
          {$t('ai.check_again')}
        </button>
      </div>
    </div>

  {:else}
    <div class="px-6 space-y-4">

      <!-- Active model section -->
      {#if ai.activeModel}
        <section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
          <h2 class="text-sm font-medium text-stone-500 mb-3">{$t('ai.active_model_heading')}</h2>
          <div class="flex items-center gap-3">
            <span
              class="text-lg"
              aria-label={ai.activeModel.quality === 'Medical' ? $t('ai.medical_model') : $t('ai.general_model')}
            >
              {ai.activeModel.quality === 'Medical' ? '\u2605' : '\u25CB'}
            </span>
            <div class="flex-1">
              <p class="text-base font-medium text-stone-800">{ai.activeModel.name}</p>
              <p class="text-xs text-stone-500">
                {ai.activeModel.quality === 'Medical' ? $t('ai.medical_trained') : $t('ai.general_purpose')}
                &middot; {sourceDisplayText(ai.activeModel.source)}
              </p>
            </div>
          </div>
        </section>
      {:else}
        <section class="bg-[var(--color-warning-50)] rounded-xl p-5 border border-[var(--color-warning-200)]">
          <h2 class="text-sm font-medium text-[var(--color-warning-800)] mb-2">{$t('ai.no_model_heading')}</h2>
          <p class="text-sm text-[var(--color-warning-800)]">{$t('ai.no_model_description')}</p>
        </section>
      {/if}

      <!-- Installed models list -->
      <section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
        <h2 class="text-sm font-medium text-stone-500 mb-3">
          {$t('ai.installed_models', { values: { count: ai.models.length } })}
        </h2>

        {#if ai.models.length === 0}
          <p class="text-sm text-stone-500 py-4 text-center">
            {$t('ai.no_models_empty')}
          </p>
        {:else}
          <div class="space-y-3" role="list" aria-label={$t('ai.installed_models_aria')}>
            {#each ai.models as model (model.name)}
              {@const isActive = ai.activeModel?.name === model.name}
              {@const medical = isMedicalModel(model.name)}
              <div
                class="flex items-center gap-3 p-3 rounded-lg border {isActive ? 'border-[var(--color-interactive)] bg-[var(--color-interactive-50)]' : 'border-stone-100'}"
                role="listitem"
                aria-current={isActive ? 'true' : undefined}
              >
                <span
                  class="text-base"
                  aria-label={medical ? $t('ai.medical_model') : $t('ai.general_model')}
                >
                  {medical ? '\u2605' : '\u25CB'}
                </span>
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-stone-800 truncate">{model.name}</p>
                  <p class="text-xs text-stone-500">
                    {medical ? $t('ai.medical_label') : $t('ai.general_label')}
                    &middot; {formatModelSize(model.size)}
                    {#if model.details.family}
                      &middot; {model.details.family}
                    {/if}
                  </p>
                </div>
                <div class="flex items-center gap-2">
                  {#if isActive}
                    <span class="text-xs font-medium text-[var(--color-interactive-hover)] bg-[var(--color-interactive-50)] px-2 py-1 rounded">{$t('ai.active_badge')}</span>
                  {:else}
                    <button
                      class="text-xs text-[var(--color-interactive-hover)] border border-[var(--color-interactive)] px-3 py-1.5 rounded-lg hover:bg-[var(--color-interactive-50)] min-h-[44px]"
                      onclick={() => handleSelectModel(model.name)}
                    >
                      {$t('ai.select_button')}
                    </button>
                  {/if}
                  <button
                    class="text-stone-500 hover:text-[var(--color-danger)] min-h-[44px] min-w-[44px] flex items-center justify-center"
                    onclick={() => { deleteConfirm = model.name; }}
                    aria-label={$t('ai.delete_model_aria', { values: { name: model.name } })}
                  >
                    &times;
                  </button>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </section>

      <!-- Pull progress -->
      {#if ai.isPulling && ai.pullProgress}
        <section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
          <h2 class="text-sm font-medium text-stone-500 mb-3">{$t('ai.downloading_heading')}</h2>
          <p class="text-sm text-stone-800 mb-2">{ai.pullProgress.model_name}</p>
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
              {ai.pullProgress.status}
              &middot; {formatModelSize(ai.pullProgress.bytes_completed)} / {formatModelSize(ai.pullProgress.bytes_total)}
              &middot; {Math.round(ai.pullProgress.progress_percent)}%
            </span>
            <button
              class="text-xs text-[var(--color-danger)] border border-[var(--color-danger-200)] px-3 py-1.5 rounded-lg hover:bg-[var(--color-danger-50)] min-h-[44px]"
              onclick={handleCancelPull}
            >
              {$t('common.cancel')}
            </button>
          </div>
        </section>
      {/if}

      <!-- Pull error -->
      {#if ai.pullProgress?.status === 'error'}
        <div class="bg-[var(--color-danger-50)] rounded-xl p-4 border border-[var(--color-danger-200)]">
          <p class="text-sm text-[var(--color-danger-800)]">
            {$t('ai.pull_failed', { values: { name: ai.pullProgress.model_name, error: ai.pullProgress.error_message ?? $t('common.unknown') } })}
          </p>
        </div>
      {/if}

      <!-- Pull section (when not actively pulling) -->
      {#if !ai.isPulling}
        <section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
          <h2 class="text-sm font-medium text-stone-500 mb-3">{$t('ai.pull_heading')}</h2>

          <!-- Custom model input -->
          <div class="flex gap-2 mb-4">
            <input
              type="text"
              bind:value={pullInput}
              placeholder={$t('ai.model_name_placeholder')}
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

          <!-- Recommended models -->
          {#if recommended.length > 0}
            <h3 class="text-xs font-medium text-stone-500 mb-2">{$t('ai.recommended_section')}</h3>
            <div class="space-y-2">
              {#each recommended as rec (rec.name)}
                {@const alreadyInstalled = ai.models.some(m => m.name === rec.name)}
                <div class="flex items-center gap-3 p-3 rounded-lg border border-stone-100">
                  <span class="text-base" aria-label={$t('ai.medical_model')}>{'\u2605'}</span>
                  <div class="flex-1 min-w-0">
                    <p class="text-sm font-medium text-stone-800">{rec.name}</p>
                    <p class="text-xs text-stone-500">
                      {rec.description} &middot; {$t('ai.requires_ram', { values: { gb: rec.min_ram_gb } })}
                    </p>
                  </div>
                  {#if alreadyInstalled}
                    <span class="text-xs text-stone-500">{$t('ai.installed_tag')}</span>
                  {:else}
                    <button
                      class="text-xs text-[var(--color-interactive-hover)] border border-[var(--color-interactive)] px-3 py-1.5 rounded-lg hover:bg-[var(--color-interactive-50)] min-h-[44px]"
                      onclick={() => handlePull(rec.name)}
                    >
                      {$t('ai.pull')}
                    </button>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        </section>
      {/if}

      <!-- Ollama status -->
      {#if ai.health}
        <section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
          <h2 class="text-sm font-medium text-stone-500 mb-3">{$t('ai.ollama_status_heading')}</h2>
          <div class="space-y-2 text-sm">
            <div class="flex justify-between">
              <span class="text-stone-600">{$t('ai.status_label')}</span>
              <span class="text-stone-800">
                {ai.health.reachable ? $t('ai.status_running') : $t('ai.status_not_running')}
              </span>
            </div>
            {#if ai.health.version}
              <div class="flex justify-between">
                <span class="text-stone-600">{$t('ai.version_label')}</span>
                <span class="text-stone-800">{ai.health.version}</span>
              </div>
            {/if}
            <div class="flex justify-between">
              <span class="text-stone-600">{$t('ai.models_count_label')}</span>
              <span class="text-stone-800">{ai.health.models_count}</span>
            </div>
          </div>
        </section>
      {/if}
    </div>
  {/if}
</div>

<!-- Delete confirmation dialog -->
{#if deleteConfirm}
  {@const isActiveDelete = ai.activeModel?.name === deleteConfirm}
  {@const modelInfo = ai.models.find(m => m.name === deleteConfirm)}
  <div
    class="fixed inset-0 bg-black/30 flex items-center justify-center z-50 p-4"
    role="alertdialog"
    aria-modal="true"
    aria-describedby="delete-desc"
  >
    <div class="bg-white rounded-2xl max-w-sm w-full p-6 shadow-xl">
      <h3 class="text-lg font-semibold text-stone-800 mb-2">{$t('ai.delete_heading')}</h3>
      <p id="delete-desc" class="text-sm text-stone-600 mb-1">
        {$t('ai.delete_confirm', { values: { name: deleteConfirm } })}
        {#if modelInfo}
          {$t('ai.delete_frees', { values: { size: formatModelSize(modelInfo.size) } })}
        {/if}
      </p>
      {#if isActiveDelete}
        <p class="text-sm text-[var(--color-warning-800)] bg-[var(--color-warning-50)] rounded-lg px-3 py-2 mt-2 mb-4">
          {$t('ai.delete_active_warning')}
        </p>
      {/if}
      <div class="flex gap-3 mt-4">
        <Button variant="secondary" onclick={() => { deleteConfirm = null; }}>
          {$t('common.cancel')}
        </Button>
        <Button variant="danger" onclick={() => handleDelete(deleteConfirm!)}>
          {$t('common.delete')}
        </Button>
      </div>
    </div>
  </div>
{/if}

<!-- Non-medical model warning -->
{#if nonMedicalWarning}
  <div
    class="fixed inset-0 bg-black/30 flex items-center justify-center z-50 p-4"
    role="alertdialog"
    aria-modal="true"
    aria-describedby="nonmed-desc"
  >
    <div class="bg-white rounded-2xl max-w-sm w-full p-6 shadow-xl">
      <h3 class="text-lg font-semibold text-[var(--color-warning-800)] mb-2">{$t('ai.nonmedical_heading')}</h3>
      <p id="nonmed-desc" class="text-sm text-stone-600">
        {$t('ai.nonmedical_description', { values: { name: nonMedicalWarning } })}
      </p>
      <p class="text-sm text-stone-500 mt-2">{$t('ai.nonmedical_privacy')}</p>
      <div class="flex gap-3 mt-4">
        <Button variant="secondary" onclick={() => { nonMedicalWarning = null; }}>
          {$t('ai.choose_medical')}
        </Button>
        <Button variant="primary" onclick={() => doSetModel(nonMedicalWarning!)}>
          {$t('ai.use_anyway')}
        </Button>
      </div>
    </div>
  </div>
{/if}
