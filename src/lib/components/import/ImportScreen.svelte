<!-- E2E-F01 + F02: File import UI — file picker, drag-and-drop, processing progress, results. -->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import { open } from '@tauri-apps/plugin-dialog';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { processDocument, processDocumentsBatch } from '$lib/api/import';
  import type {
    ProcessingOutcome,
    ProcessingProgressEvent,
    ProcessingBatchProgressEvent,
  } from '$lib/types/import';
  import { navigation } from '$lib/stores/navigation.svelte';
  import BackButton from '$lib/components/ui/BackButton.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import Divider from '$lib/components/ui/Divider.svelte';

  interface Props {
    droppedFiles?: string;
  }

  let { droppedFiles }: Props = $props();

  /** Supported medical document extensions (reactive for i18n). */
  let documentFilters = $derived([
    { name: $t('import.filter_medical'), extensions: ['pdf', 'jpg', 'jpeg', 'png', 'tiff', 'tif', 'txt'] },
    { name: $t('import.filter_pdf'), extensions: ['pdf'] },
    { name: $t('import.filter_images'), extensions: ['jpg', 'jpeg', 'png', 'tiff', 'tif'] },
  ]);

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

  // GAP-H01: Drag-and-drop state
  let isDragging = $state(false);
  let unlistenDragDrop: (() => void) | null = null;

  const SUPPORTED_EXTENSIONS = ['pdf', 'jpg', 'jpeg', 'png', 'tiff', 'tif', 'txt'];

  function filterSupportedPaths(paths: string[]): string[] {
    return paths.filter((p) => {
      const ext = p.split('.').pop()?.toLowerCase() ?? '';
      return SUPPORTED_EXTENSIONS.includes(ext);
    });
  }

  onMount(async () => {
    // Auto-process files dropped from home screen
    if (droppedFiles) {
      const paths = droppedFiles.split('|').filter(Boolean);
      if (paths.length > 0) {
        processFiles(paths);
      }
    }

    const webview = getCurrentWebview();
    unlistenDragDrop = await webview.onDragDropEvent((event) => {
      if (screen !== 'idle') return; // Only accept drops on idle screen

      const payload = event.payload;
      if (payload.type === 'enter') {
        isDragging = true;
      } else if (payload.type === 'leave') {
        isDragging = false;
      } else if (payload.type === 'drop') {
        isDragging = false;
        const supported = filterSupportedPaths(payload.paths);
        if (supported.length > 0) {
          processFiles(supported);
        }
      }
    });
  });

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
    unlistenDragDrop?.();
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
      title: $t('import.dialog_title'),
      multiple: true,
      filters: documentFilters,
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
    <BackButton />
    <h1 class="text-lg font-semibold text-stone-800">{$t('import.heading')}</h1>
  </header>

  <!-- Content -->
  <div class="flex-1 flex flex-col items-center justify-center px-6 py-8">

    {#if screen === 'idle'}
      <!-- File selection UI with drop zone -->
      <div class="w-full max-w-md relative">
        <!-- GAP-H01: Drag overlay -->
        {#if isDragging}
          <div
            class="absolute inset-0 z-10 border-2 border-dashed border-[var(--color-interactive)]
                   bg-[var(--color-interactive-50)]/80 rounded-2xl flex flex-col items-center justify-center
                   pointer-events-none"
            role="status"
            aria-live="assertive"
          >
            <div class="w-16 h-16 bg-[var(--color-interactive-50)] rounded-2xl flex items-center justify-center mb-4">
              <span class="text-3xl text-[var(--color-interactive)]">&darr;</span>
            </div>
            <p class="text-lg font-semibold text-[var(--color-interactive-hover)]">{$t('import.drop_files_here')}</p>
            <p class="text-sm text-[var(--color-interactive)] mt-1">{$t('import.supported_formats')}</p>
          </div>
        {/if}

        <div class="text-center mb-8">
          <div class="w-16 h-16 bg-[var(--color-interactive-50)] rounded-2xl flex items-center justify-center mx-auto mb-4">
            <span class="text-3xl text-[var(--color-interactive)]">+</span>
          </div>
          <h2 class="text-xl font-semibold text-stone-800 mb-2">{$t('import.add_documents')}</h2>
          <p class="text-sm text-stone-500">
            {$t('import.description')}
          </p>
        </div>

        <!-- Browse button -->
        <div class="mb-3">
          <Button variant="primary" fullWidth onclick={browseFiles}>
            {$t('import.browse_files')}
          </Button>
        </div>

        <p class="text-xs text-stone-500 text-center mb-4">
          {$t('import.supported_formats')}
        </p>

        <!-- GAP-H01: Drop hint -->
        <p class="text-xs text-stone-400 text-center mb-4">
          {$t('import.drag_drop_hint')}
        </p>

        <!-- Divider -->
        <div class="mb-6">
          <Divider label={$t('import.or')} />
        </div>

        <!-- WiFi transfer option -->
        <Button variant="secondary" fullWidth onclick={() => navigation.navigate('transfer')}>
          {$t('import.receive_from_phone')}
        </Button>
        <p class="text-xs text-stone-500 text-center mt-2">
          {$t('import.receive_description')}
        </p>
      </div>

    {:else if screen === 'processing'}
      <!-- Processing progress -->
      <div class="w-full max-w-md text-center">
        <!-- Q.4: Pulse animation during structuring, spinner otherwise -->
        <div class="w-16 h-16 mx-auto mb-6 relative">
          {#if progressStage === 'structuring'}
            <div class="w-16 h-16 rounded-full bg-[var(--color-interactive-50)] animate-pulse flex items-center
                        justify-center">
              <div class="w-10 h-10 rounded-full bg-[var(--color-interactive-50)] animate-pulse"
                   style="animation-delay: 200ms"></div>
            </div>
          {:else}
            <div class="animate-spin w-16 h-16 border-3 border-[var(--color-interactive-50)]
                        border-t-[var(--color-interactive)] rounded-full"></div>
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

        <p class="text-sm font-medium text-[var(--color-interactive-hover)] mb-4">
          {stageLabel(progressStage)}
        </p>

        <!-- Q.4: Indeterminate bar during structuring, determinate otherwise -->
        {#if progressStage === 'structuring'}
          <div class="w-full bg-stone-200 rounded-full h-2 mb-2 overflow-hidden">
            <div class="h-2 bg-[var(--color-interactive)] rounded-full animate-indeterminate"></div>
          </div>
          <p class="text-xs text-stone-500">{$t('import.analyzing_ai')}</p>
        {:else}
          <div class="w-full bg-stone-200 rounded-full h-2 mb-2">
            <div
              class="bg-[var(--color-interactive)] h-2 rounded-full transition-all duration-300"
              style="width: {progressPct}%"
            ></div>
          </div>
          <p class="text-xs text-stone-500">{progressPct}%</p>
        {/if}

        <!-- Q.3: Elapsed timer -->
        <p class="text-xs text-stone-500 mt-4 tabular-nums" aria-live="off">
          {$t('import.elapsed', { values: { time: formatElapsed(elapsedSeconds) } })}
        </p>

        <p class="text-xs text-stone-500 mt-2">
          {$t('import.ai_analysis_note')}
        </p>

        <!-- Q.5: Cancel button -->
        <div class="mt-6">
          <Button variant="ghost" onclick={handleCancel}>
            {$t('common.cancel')}
          </Button>
        </div>
      </div>

    {:else if screen === 'success'}
      <!-- Success results -->
      <div class="w-full max-w-md">
        <div class="text-center mb-6">
          <div class="w-16 h-16 bg-[var(--color-success-50)] rounded-2xl flex items-center justify-center mx-auto mb-4">
            <span class="text-3xl text-[var(--color-success)]">&#x2713;</span>
          </div>
          <h2 class="text-xl font-semibold text-stone-800 mb-1">
            {successCount === 1
              ? $t('import.success_single')
              : $t('import.success_plural', { values: { count: successCount } })}
          </h2>
          {#if failureCount > 0}
            <p class="text-sm text-[var(--color-warning)]">
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
                       ? 'bg-[var(--color-success-50)] border border-[var(--color-success-50)]'
                       : 'bg-[var(--color-danger-50)] border border-[var(--color-danger-50)]'}"
            >
              <span class="{outcome.import_status === 'Staged' ? 'text-[var(--color-success)]' : 'text-[var(--color-danger)]'} text-sm">
                {outcome.import_status === 'Staged' ? '\u2713' : '\u2717'}
              </span>
              <div class="flex-1 min-w-0">
                <p class="text-sm text-stone-700 truncate">{outcome.original_filename}</p>
                {#if outcome.structuring}
                  <p class="text-xs text-stone-500">
                    {outcome.structuring.document_type}
                    {#if outcome.structuring.entities_count > 0}
                      &middot; {$t('import.entities_found', { values: { count: outcome.structuring.entities_count } })}
                    {/if}
                  </p>
                {:else if outcome.import_status !== 'Staged'}
                  <p class="text-xs text-[var(--color-danger)]">{outcome.import_status}</p>
                {/if}
              </div>
              {#if outcome.import_status === 'Staged'}
                <button
                  class="text-xs text-[var(--color-interactive)] font-medium min-h-[44px] min-w-[44px]
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
        <div class="flex flex-col gap-3">
          {#if successCount === 1}
            <Button variant="primary" fullWidth onclick={() => navigateToReview(outcomes.find((o) => o.import_status === 'Staged')!.document_id)}>
              {$t('import.review_document')}
            </Button>
          {/if}

          <Button variant="secondary" fullWidth onclick={reset}>
            {$t('import.import_more')}
          </Button>

          <Button variant="ghost" fullWidth onclick={() => navigation.navigate('home')}>
            {$t('import.back_to_home')}
          </Button>
        </div>
      </div>

    {:else if screen === 'error'}
      <!-- Error state -->
      <div class="w-full max-w-md text-center">
        <div class="w-16 h-16 bg-[var(--color-danger-50)] rounded-2xl flex items-center justify-center mx-auto mb-4">
          <span class="text-3xl text-[var(--color-danger)]">!</span>
        </div>
        <h2 class="text-lg font-semibold text-stone-800 mb-2">{$t('import.error_heading')}</h2>
        <p class="text-sm text-[var(--color-danger)] mb-6">{errorMessage}</p>

        <div class="flex flex-col gap-3">
          <Button variant="primary" fullWidth onclick={reset}>
            {$t('common.retry')}
          </Button>
          <Button variant="ghost" fullWidth onclick={() => navigation.navigate('home')}>
            {$t('import.back_to_home')}
          </Button>
        </div>
      </div>
    {/if}
  </div>
</div>
