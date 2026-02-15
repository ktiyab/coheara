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
    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center rounded-lg hover:bg-stone-100"
      onclick={() => navigation.goBack()}
      aria-label="Go back"
    >
      <span class="text-xl text-stone-600">&larr;</span>
    </button>
    <h1 class="text-2xl font-bold text-stone-800">AI Engine</h1>
  </header>

  {#if ai.loading}
    <!-- Loading state -->
    <div class="flex-1 flex items-center justify-center">
      <div class="animate-pulse text-stone-400">Loading AI settings...</div>
    </div>

  {:else if ai.error}
    <!-- Error state -->
    <div class="px-6">
      <div class="bg-red-50 rounded-xl p-5 border border-red-200">
        <p class="text-sm text-red-700">{ai.error}</p>
        <button
          class="mt-3 px-4 py-2 bg-white border border-red-200 rounded-lg text-sm text-red-700 hover:bg-red-50 min-h-[44px]"
          onclick={handleRetry}
        >
          Retry
        </button>
      </div>
    </div>

  {:else if !ai.isOllamaReachable}
    <!-- Ollama not running -->
    <div class="px-6 space-y-4">
      <div class="bg-amber-50 rounded-xl p-5 border border-amber-200">
        <h2 class="text-base font-medium text-amber-800 mb-2">Ollama is not running</h2>
        <p class="text-sm text-amber-700 mb-4">
          Coheara needs Ollama to run AI models locally on your computer.
          Install Ollama and start it, then come back here.
        </p>
        <div class="space-y-2 text-sm text-stone-600">
          <p><strong>Install:</strong> Visit <span class="font-mono text-stone-800">ollama.com/download</span></p>
          <p><strong>Start:</strong> Open Ollama from your applications</p>
        </div>
        <button
          class="mt-4 px-4 py-2 bg-amber-100 border border-amber-300 rounded-lg text-sm text-amber-800 hover:bg-amber-200 min-h-[44px]"
          onclick={handleRetry}
        >
          Check again
        </button>
      </div>
    </div>

  {:else}
    <div class="px-6 space-y-4">

      <!-- Active model section -->
      {#if ai.activeModel}
        <section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
          <h2 class="text-sm font-medium text-stone-500 mb-3">ACTIVE MODEL</h2>
          <div class="flex items-center gap-3">
            <span
              class="text-lg"
              aria-label={ai.activeModel.quality === 'Medical' ? 'Medical model' : 'General model'}
            >
              {ai.activeModel.quality === 'Medical' ? '\u2605' : '\u25CB'}
            </span>
            <div class="flex-1">
              <p class="text-base font-medium text-stone-800">{ai.activeModel.name}</p>
              <p class="text-xs text-stone-500">
                {ai.activeModel.quality === 'Medical' ? 'Medical-trained' : 'General-purpose'}
                &middot; {sourceDisplayText(ai.activeModel.source)}
              </p>
            </div>
          </div>
        </section>
      {:else}
        <section class="bg-amber-50 rounded-xl p-5 border border-amber-200">
          <h2 class="text-sm font-medium text-amber-800 mb-2">No AI model selected</h2>
          <p class="text-sm text-amber-700">Pull a model below to enable AI features.</p>
        </section>
      {/if}

      <!-- Installed models list -->
      <section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
        <h2 class="text-sm font-medium text-stone-500 mb-3">
          INSTALLED MODELS ({ai.models.length})
        </h2>

        {#if ai.models.length === 0}
          <p class="text-sm text-stone-400 py-4 text-center">
            No models installed. Pull one from the recommendations below.
          </p>
        {:else}
          <div class="space-y-3" role="list" aria-label="Installed AI models">
            {#each ai.models as model (model.name)}
              {@const isActive = ai.activeModel?.name === model.name}
              {@const medical = isMedicalModel(model.name)}
              <div
                class="flex items-center gap-3 p-3 rounded-lg border {isActive ? 'border-teal-200 bg-teal-50' : 'border-stone-100'}"
                role="listitem"
                aria-current={isActive ? 'true' : undefined}
              >
                <span
                  class="text-base"
                  aria-label={medical ? 'Medical model' : 'General model'}
                >
                  {medical ? '\u2605' : '\u25CB'}
                </span>
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-stone-800 truncate">{model.name}</p>
                  <p class="text-xs text-stone-500">
                    {medical ? 'Medical' : 'General'}
                    &middot; {formatModelSize(model.size)}
                    {#if model.details.family}
                      &middot; {model.details.family}
                    {/if}
                  </p>
                </div>
                <div class="flex items-center gap-2">
                  {#if isActive}
                    <span class="text-xs font-medium text-teal-700 bg-teal-100 px-2 py-1 rounded">ACTIVE</span>
                  {:else}
                    <button
                      class="text-xs text-teal-700 border border-teal-200 px-3 py-1.5 rounded-lg hover:bg-teal-50 min-h-[44px]"
                      onclick={() => handleSelectModel(model.name)}
                    >
                      Select
                    </button>
                  {/if}
                  <button
                    class="text-stone-400 hover:text-red-500 min-h-[44px] min-w-[44px] flex items-center justify-center"
                    onclick={() => { deleteConfirm = model.name; }}
                    aria-label={`Delete ${model.name}`}
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
          <h2 class="text-sm font-medium text-stone-500 mb-3">DOWNLOADING</h2>
          <p class="text-sm text-stone-800 mb-2">{ai.pullProgress.model_name}</p>
          <div
            class="w-full bg-stone-200 rounded-full h-2.5 mb-2"
            role="progressbar"
            aria-valuenow={Math.round(ai.pullProgress.progress_percent)}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label={`Downloading ${ai.pullProgress.model_name}: ${Math.round(ai.pullProgress.progress_percent)}%`}
          >
            <div
              class="bg-teal-500 h-2.5 rounded-full transition-all"
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
              class="text-xs text-red-600 border border-red-200 px-3 py-1.5 rounded-lg hover:bg-red-50 min-h-[44px]"
              onclick={handleCancelPull}
            >
              Cancel
            </button>
          </div>
        </section>
      {/if}

      <!-- Pull error -->
      {#if ai.pullProgress?.status === 'error'}
        <div class="bg-red-50 rounded-xl p-4 border border-red-200">
          <p class="text-sm text-red-700">
            Failed to pull {ai.pullProgress.model_name}: {ai.pullProgress.error_message ?? 'Unknown error'}
          </p>
        </div>
      {/if}

      <!-- Pull section (when not actively pulling) -->
      {#if !ai.isPulling}
        <section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
          <h2 class="text-sm font-medium text-stone-500 mb-3">PULL A MODEL</h2>

          <!-- Custom model input -->
          <div class="flex gap-2 mb-4">
            <input
              type="text"
              bind:value={pullInput}
              placeholder="Model name (e.g., medgemma:4b)"
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

          <!-- Recommended models -->
          {#if recommended.length > 0}
            <h3 class="text-xs font-medium text-stone-400 mb-2">RECOMMENDED</h3>
            <div class="space-y-2">
              {#each recommended as rec (rec.name)}
                {@const alreadyInstalled = ai.models.some(m => m.name === rec.name)}
                <div class="flex items-center gap-3 p-3 rounded-lg border border-stone-100">
                  <span class="text-base" aria-label="Medical model">{'\u2605'}</span>
                  <div class="flex-1 min-w-0">
                    <p class="text-sm font-medium text-stone-800">{rec.name}</p>
                    <p class="text-xs text-stone-500">
                      {rec.description} &middot; {rec.min_ram_gb}GB+ RAM
                    </p>
                  </div>
                  {#if alreadyInstalled}
                    <span class="text-xs text-stone-400">Installed</span>
                  {:else}
                    <button
                      class="text-xs text-teal-700 border border-teal-200 px-3 py-1.5 rounded-lg hover:bg-teal-50 min-h-[44px]"
                      onclick={() => handlePull(rec.name)}
                    >
                      Pull
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
          <h2 class="text-sm font-medium text-stone-500 mb-3">OLLAMA STATUS</h2>
          <div class="space-y-2 text-sm">
            <div class="flex justify-between">
              <span class="text-stone-600">Status</span>
              <span class="text-stone-800">
                {ai.health.reachable ? 'Running' : 'Not running'}
              </span>
            </div>
            {#if ai.health.version}
              <div class="flex justify-between">
                <span class="text-stone-600">Version</span>
                <span class="text-stone-800">{ai.health.version}</span>
              </div>
            {/if}
            <div class="flex justify-between">
              <span class="text-stone-600">Models installed</span>
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
      <h3 class="text-lg font-semibold text-stone-800 mb-2">Delete model?</h3>
      <p id="delete-desc" class="text-sm text-stone-600 mb-1">
        Delete <strong>{deleteConfirm}</strong>?
        {#if modelInfo}
          This frees {formatModelSize(modelInfo.size)}.
        {/if}
      </p>
      {#if isActiveDelete}
        <p class="text-sm text-amber-700 bg-amber-50 rounded-lg px-3 py-2 mt-2 mb-4">
          This is your active AI model. Coheara will switch to the next available model.
        </p>
      {/if}
      <div class="flex gap-3 mt-4">
        <button
          class="flex-1 px-4 py-2 border border-stone-200 rounded-lg text-sm text-stone-600 hover:bg-stone-50 min-h-[44px]"
          onclick={() => { deleteConfirm = null; }}
        >
          Cancel
        </button>
        <button
          class="flex-1 px-4 py-2 bg-red-600 text-white rounded-lg text-sm hover:bg-red-700 min-h-[44px]"
          onclick={() => handleDelete(deleteConfirm!)}
        >
          Delete
        </button>
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
      <h3 class="text-lg font-semibold text-amber-800 mb-2">Non-medical model</h3>
      <p id="nonmed-desc" class="text-sm text-stone-600">
        <strong>{nonMedicalWarning}</strong> is a general-purpose model. It may be less
        accurate for medical document analysis than a medical-trained model.
      </p>
      <p class="text-sm text-stone-500 mt-2">Your data remains private and secure regardless.</p>
      <div class="flex gap-3 mt-4">
        <button
          class="flex-1 px-4 py-2 border border-stone-200 rounded-lg text-sm text-stone-600 hover:bg-stone-50 min-h-[44px]"
          onclick={() => { nonMedicalWarning = null; }}
        >
          Choose medical model
        </button>
        <button
          class="flex-1 px-4 py-2 bg-amber-600 text-white rounded-lg text-sm hover:bg-amber-700 min-h-[44px]"
          onclick={() => doSetModel(nonMedicalWarning!)}
        >
          Use anyway
        </button>
      </div>
    </div>
  </div>
{/if}
