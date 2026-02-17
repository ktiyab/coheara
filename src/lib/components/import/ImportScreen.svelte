<!-- E2E-F01 + F02: File import UI — file picker, processing progress, results. -->
<script lang="ts">
  import { onDestroy } from 'svelte';
  import { t } from 'svelte-i18n';
  import { open } from '@tauri-apps/plugin-dialog';
  import { listen } from '@tauri-apps/api/event';
  import { processDocument, processDocumentsBatch } from '$lib/api/import';
  import type {
    ProcessingOutcome,
    ProcessingProgressEvent,
    ProcessingBatchProgressEvent,
  } from '$lib/types/import';
  import { navigation } from '$lib/stores/navigation.svelte';

  /** Supported medical document extensions. */
  const DOCUMENT_FILTERS = [
    { name: 'Medical Documents', extensions: ['pdf', 'jpg', 'jpeg', 'png', 'tiff', 'tif', 'txt'] },
    { name: 'PDF', extensions: ['pdf'] },
    { name: 'Images', extensions: ['jpg', 'jpeg', 'png', 'tiff', 'tif'] },
  ];

  type ScreenState = 'idle' | 'processing' | 'success' | 'error';

  let screen: ScreenState = $state('idle');
  let progressStage = $state('');
  let progressPct = $state(0);
  let progressFileName = $state('');
  let batchCurrent = $state(0);
  let batchTotal = $state(0);
  let outcomes: ProcessingOutcome[] = $state([]);
  let errorMessage: string | null = $state(null);

  // Q.3: Elapsed timer state
  let elapsedSeconds = $state(0);
  let elapsedTimer: ReturnType<typeof setInterval> | null = null;

  // Q.5: Cancel state (user abandoned processing)
  let cancelled = $state(false);

  let unlistenProgress: (() => void) | null = null;
  let unlistenBatch: (() => void) | null = null;

  // Q.3: Format elapsed seconds as M:SS or H:MM:SS
  function formatElapsed(seconds: number): string {
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = seconds % 60;
    if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
    return `${m}:${String(s).padStart(2, '0')}`;
  }

  // Q.3: Start elapsed timer
  function startElapsedTimer() {
    elapsedSeconds = 0;
    elapsedTimer = setInterval(() => { elapsedSeconds += 1; }, 1000);
  }

  // Q.3: Stop elapsed timer
  function stopElapsedTimer() {
    if (elapsedTimer) {
      clearInterval(elapsedTimer);
      elapsedTimer = null;
    }
  }

  onDestroy(() => {
    unlistenProgress?.();
    unlistenBatch?.();
    stopElapsedTimer();
  });

  /** Human-readable stage labels. */
  function stageLabel(stage: string): string {
    switch (stage) {
      case 'importing': return $t('import.stage_importing');
      case 'extracting': return $t('import.stage_extracting');
      case 'structuring': return $t('import.stage_structuring');
      case 'saving_review': return $t('import.stage_saving');
      case 'complete': return $t('import.stage_complete');
      case 'failed': return $t('import.stage_failed');
      default: return stage;
    }
  }

  async function setupListeners() {
    unlistenProgress?.();
    unlistenBatch?.();

    const u1 = await listen<ProcessingProgressEvent>('processing-progress', (event) => {
      const p = event.payload;
      progressStage = p.stage;
      progressFileName = p.file_name;
      if (p.progress_pct !== null) {
        progressPct = p.progress_pct;
      }
      if (p.stage === 'failed' && p.error) {
        errorMessage = p.error;
      }
    });
    unlistenProgress = u1;

    const u2 = await listen<ProcessingBatchProgressEvent>('processing-batch-progress', (event) => {
      const p = event.payload;
      batchCurrent = p.current;
      batchTotal = p.total;
      progressFileName = p.file_name;
      progressStage = p.stage;
    });
    unlistenBatch = u2;
  }

  async function browseFiles() {
    const selected = await open({
      title: 'Select medical documents',
      multiple: true,
      filters: DOCUMENT_FILTERS,
    });

    if (!selected) return; // User cancelled

    const paths = Array.isArray(selected) ? selected : [selected];
    if (paths.length === 0) return;

    await processFiles(paths);
  }

  async function processFiles(paths: string[]) {
    screen = 'processing';
    errorMessage = null;
    progressStage = 'importing';
    progressPct = 0;
    progressFileName = '';
    batchCurrent = 0;
    batchTotal = paths.length;
    outcomes = [];
    cancelled = false;
    startElapsedTimer();

    await setupListeners();

    try {
      if (paths.length === 1) {
        const outcome = await processDocument(paths[0]);
        outcomes = [outcome];
      } else {
        outcomes = await processDocumentsBatch(paths);
      }

      stopElapsedTimer();

      // Q.5: If user cancelled while backend was processing, stay on idle
      if (cancelled) return;

      const successful = outcomes.filter((o) => o.import_status === 'Staged');
      if (successful.length > 0) {
        screen = 'success';
      } else {
        screen = 'error';
        errorMessage = errorMessage ?? $t('import.no_files_error');
      }
    } catch (e) {
      stopElapsedTimer();
      if (cancelled) return;
      screen = 'error';
      errorMessage = e instanceof Error ? e.message : String(e);
    }
  }

  function navigateToReview(documentId: string) {
    navigation.navigate('review', { documentId });
  }

  // Q.5: Cancel processing — abandon the UI wait (backend continues to completion)
  function handleCancel() {
    cancelled = true;
    stopElapsedTimer();
    reset();
  }

  function reset() {
    screen = 'idle';
    progressStage = '';
    progressPct = 0;
    progressFileName = '';
    batchCurrent = 0;
    batchTotal = 0;
    outcomes = [];
    errorMessage = null;
    elapsedSeconds = 0;
  }

  let successCount = $derived(outcomes.filter((o) => o.import_status === 'Staged').length);
  let failureCount = $derived(outcomes.length - successCount);
</script>

<div class="flex flex-col min-h-screen bg-stone-50">
  <!-- Header -->
  <header class="flex items-center gap-3 px-4 py-3 bg-white border-b border-stone-200 shrink-0">
    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center
             text-stone-500 hover:text-stone-700"
      onclick={() => navigation.goBack()}
      aria-label={$t('nav.go_back')}
    >
      &larr;
    </button>
    <h1 class="text-lg font-semibold text-stone-800">{$t('import.heading')}</h1>
  </header>

  <!-- Content -->
  <div class="flex-1 flex flex-col items-center justify-center px-6 py-8">

    {#if screen === 'idle'}
      <!-- File selection UI -->
      <div class="w-full max-w-md">
        <div class="text-center mb-8">
          <div class="w-16 h-16 bg-teal-50 rounded-2xl flex items-center justify-center mx-auto mb-4">
            <span class="text-3xl text-teal-600">+</span>
          </div>
          <h2 class="text-xl font-semibold text-stone-800 mb-2">{$t('import.add_documents')}</h2>
          <p class="text-sm text-stone-500">
            {$t('import.description')}
          </p>
        </div>

        <!-- Browse button -->
        <button
          class="w-full px-6 py-4 bg-[var(--color-primary)] text-white rounded-xl
                 text-base font-medium min-h-[44px] mb-3"
          onclick={browseFiles}
        >
          {$t('import.browse_files')}
        </button>

        <p class="text-xs text-stone-400 text-center mb-8">
          {$t('import.supported_formats')}
        </p>

        <!-- Divider -->
        <div class="flex items-center gap-4 mb-6">
          <div class="flex-1 h-px bg-stone-200"></div>
          <span class="text-xs text-stone-400">{$t('import.or')}</span>
          <div class="flex-1 h-px bg-stone-200"></div>
        </div>

        <!-- WiFi transfer option -->
        <button
          class="w-full px-6 py-4 bg-white border border-stone-200 rounded-xl
                 text-base text-stone-700 min-h-[44px] hover:bg-stone-50"
          onclick={() => navigation.navigate('transfer')}
        >
          {$t('import.receive_from_phone')}
        </button>
        <p class="text-xs text-stone-400 text-center mt-2">
          {$t('import.receive_description')}
        </p>
      </div>

    {:else if screen === 'processing'}
      <!-- Processing progress -->
      <div class="w-full max-w-md text-center">
        <!-- Q.4: Pulse animation during structuring, spinner otherwise -->
        <div class="w-16 h-16 mx-auto mb-6 relative">
          {#if progressStage === 'structuring'}
            <div class="w-16 h-16 rounded-full bg-teal-100 animate-pulse flex items-center
                        justify-center">
              <div class="w-10 h-10 rounded-full bg-teal-200 animate-pulse"
                   style="animation-delay: 200ms"></div>
            </div>
          {:else}
            <div class="animate-spin w-16 h-16 border-3 border-teal-200
                        border-t-teal-600 rounded-full"></div>
          {/if}
        </div>

        <h2 class="text-lg font-semibold text-stone-800 mb-1">
          {batchTotal > 1
            ? $t('import.processing_batch', { values: { current: batchCurrent, total: batchTotal } })
            : $t('import.processing_title')}
        </h2>

        {#if progressFileName}
          <p class="text-sm text-stone-500 mb-4 truncate">
            {progressFileName}
          </p>
        {/if}

        <p class="text-sm font-medium text-teal-700 mb-4">
          {stageLabel(progressStage)}
        </p>

        <!-- Q.4: Indeterminate bar during structuring, determinate otherwise -->
        {#if progressStage === 'structuring'}
          <div class="w-full bg-stone-200 rounded-full h-2 mb-2 overflow-hidden">
            <div class="h-2 bg-teal-600 rounded-full animate-indeterminate"></div>
          </div>
          <p class="text-xs text-stone-400">{$t('import.analyzing_ai')}</p>
        {:else}
          <div class="w-full bg-stone-200 rounded-full h-2 mb-2">
            <div
              class="bg-teal-600 h-2 rounded-full transition-all duration-300"
              style="width: {progressPct}%"
            ></div>
          </div>
          <p class="text-xs text-stone-400">{progressPct}%</p>
        {/if}

        <!-- Q.3: Elapsed timer -->
        <p class="text-xs text-stone-400 mt-4 tabular-nums" aria-live="off">
          {$t('import.elapsed', { values: { time: formatElapsed(elapsedSeconds) } })}
        </p>

        <p class="text-xs text-stone-400 mt-2">
          {$t('import.ai_analysis_note')}
        </p>

        <!-- Q.5: Cancel button -->
        <button
          class="mt-6 px-6 py-3 text-sm text-stone-500 hover:text-stone-700
                 border border-stone-200 rounded-xl min-h-[44px]
                 hover:bg-stone-50 transition-colors"
          onclick={handleCancel}
        >
          {$t('common.cancel')}
        </button>
      </div>

    {:else if screen === 'success'}
      <!-- Success results -->
      <div class="w-full max-w-md">
        <div class="text-center mb-6">
          <div class="w-16 h-16 bg-green-50 rounded-2xl flex items-center justify-center mx-auto mb-4">
            <span class="text-3xl text-green-600">&#x2713;</span>
          </div>
          <h2 class="text-xl font-semibold text-stone-800 mb-1">
            {successCount === 1
              ? $t('import.success_single')
              : $t('import.success_plural', { values: { count: successCount } })}
          </h2>
          {#if failureCount > 0}
            <p class="text-sm text-amber-600">
              {$t('import.failures_note', { values: { count: failureCount } })}
            </p>
          {/if}
        </div>

        <!-- Results list -->
        <div class="space-y-2 mb-6">
          {#each outcomes as outcome}
            <div
              class="flex items-center gap-3 px-4 py-3 rounded-xl
                     {outcome.import_status === 'Staged'
                       ? 'bg-green-50 border border-green-100'
                       : 'bg-red-50 border border-red-100'}"
            >
              <span class="{outcome.import_status === 'Staged' ? 'text-green-600' : 'text-red-500'} text-sm">
                {outcome.import_status === 'Staged' ? '\u2713' : '\u2717'}
              </span>
              <div class="flex-1 min-w-0">
                <p class="text-sm text-stone-700 truncate">{outcome.original_filename}</p>
                {#if outcome.structuring}
                  <p class="text-xs text-stone-500">
                    {outcome.structuring.document_type}
                    {#if outcome.structuring.entities_count > 0}
                      &middot; {outcome.structuring.entities_count} entities found
                    {/if}
                  </p>
                {:else if outcome.import_status !== 'Staged'}
                  <p class="text-xs text-red-500">{outcome.import_status}</p>
                {/if}
              </div>
              {#if outcome.import_status === 'Staged'}
                <button
                  class="text-xs text-teal-600 font-medium min-h-[44px] min-w-[44px]
                         flex items-center justify-center"
                  onclick={() => navigateToReview(outcome.document_id)}
                >
                  {$t('import.review')}
                </button>
              {/if}
            </div>
          {/each}
        </div>

        <!-- Actions -->
        {#if successCount === 1}
          <button
            class="w-full px-6 py-4 bg-[var(--color-primary)] text-white rounded-xl
                   text-base font-medium min-h-[44px] mb-3"
            onclick={() => navigateToReview(outcomes.find((o) => o.import_status === 'Staged')!.document_id)}
          >
            {$t('import.review_document')}
          </button>
        {/if}

        <button
          class="w-full px-6 py-4 bg-white border border-stone-200 rounded-xl
                 text-base text-stone-700 min-h-[44px] mb-3"
          onclick={reset}
        >
          {$t('import.import_more')}
        </button>

        <button
          class="w-full text-sm text-stone-500 min-h-[44px]"
          onclick={() => navigation.navigate('home')}
        >
          {$t('import.back_to_home')}
        </button>
      </div>

    {:else if screen === 'error'}
      <!-- Error state -->
      <div class="w-full max-w-md text-center">
        <div class="w-16 h-16 bg-red-50 rounded-2xl flex items-center justify-center mx-auto mb-4">
          <span class="text-3xl text-red-500">!</span>
        </div>
        <h2 class="text-lg font-semibold text-stone-800 mb-2">{$t('import.error_heading')}</h2>
        <p class="text-sm text-red-600 mb-6">{errorMessage}</p>

        <button
          class="w-full px-6 py-4 bg-[var(--color-primary)] text-white rounded-xl
                 text-base font-medium min-h-[44px] mb-3"
          onclick={reset}
        >
          {$t('common.retry')}
        </button>
        <button
          class="w-full text-sm text-stone-500 min-h-[44px]"
          onclick={() => navigation.navigate('home')}
        >
          {$t('import.back_to_home')}
        </button>
      </div>
    {/if}
  </div>
</div>
