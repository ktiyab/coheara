<!-- E2E-F01 + F02: File import UI — file picker, processing progress, results. -->
<script lang="ts">
  import { onDestroy } from 'svelte';
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

  let unlistenProgress: (() => void) | null = null;
  let unlistenBatch: (() => void) | null = null;

  onDestroy(() => {
    unlistenProgress?.();
    unlistenBatch?.();
  });

  /** Human-readable stage labels. */
  function stageLabel(stage: string): string {
    switch (stage) {
      case 'importing': return 'Importing file...';
      case 'extracting': return 'Extracting text...';
      case 'structuring': return 'Analyzing with AI...';
      case 'saving_review': return 'Preparing for review...';
      case 'complete': return 'Complete';
      case 'failed': return 'Failed';
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

    await setupListeners();

    try {
      if (paths.length === 1) {
        const outcome = await processDocument(paths[0]);
        outcomes = [outcome];
      } else {
        outcomes = await processDocumentsBatch(paths);
      }

      const successful = outcomes.filter((o) => o.import_status === 'Staged');
      if (successful.length > 0) {
        screen = 'success';
      } else {
        screen = 'error';
        errorMessage = errorMessage ?? 'No files could be processed.';
      }
    } catch (e) {
      screen = 'error';
      errorMessage = e instanceof Error ? e.message : String(e);
    }
  }

  function navigateToReview(documentId: string) {
    navigation.navigate('review', { documentId });
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
      aria-label="Go back"
    >
      &larr;
    </button>
    <h1 class="text-lg font-semibold text-stone-800">Import Documents</h1>
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
          <h2 class="text-xl font-semibold text-stone-800 mb-2">Add medical documents</h2>
          <p class="text-sm text-stone-500">
            Import lab results, prescriptions, imaging reports, or clinical notes.
            Coheara will extract and organize the information.
          </p>
        </div>

        <!-- Browse button -->
        <button
          class="w-full px-6 py-4 bg-[var(--color-primary)] text-white rounded-xl
                 text-base font-medium min-h-[44px] mb-3"
          onclick={browseFiles}
        >
          Browse files
        </button>

        <p class="text-xs text-stone-400 text-center mb-8">
          Supports PDF, JPEG, PNG, TIFF, and text files
        </p>

        <!-- Divider -->
        <div class="flex items-center gap-4 mb-6">
          <div class="flex-1 h-px bg-stone-200"></div>
          <span class="text-xs text-stone-400">or</span>
          <div class="flex-1 h-px bg-stone-200"></div>
        </div>

        <!-- WiFi transfer option -->
        <button
          class="w-full px-6 py-4 bg-white border border-stone-200 rounded-xl
                 text-base text-stone-700 min-h-[44px] hover:bg-stone-50"
          onclick={() => navigation.navigate('transfer')}
        >
          Receive from phone
        </button>
        <p class="text-xs text-stone-400 text-center mt-2">
          Scan a QR code to send documents from your phone
        </p>
      </div>

    {:else if screen === 'processing'}
      <!-- Processing progress -->
      <div class="w-full max-w-md text-center">
        <div class="w-16 h-16 mx-auto mb-6 relative">
          <div class="animate-spin w-16 h-16 border-3 border-teal-200
                      border-t-teal-600 rounded-full"></div>
        </div>

        <h2 class="text-lg font-semibold text-stone-800 mb-1">
          Processing{batchTotal > 1 ? ` (${batchCurrent}/${batchTotal})` : ''}
        </h2>

        {#if progressFileName}
          <p class="text-sm text-stone-500 mb-4 truncate">
            {progressFileName}
          </p>
        {/if}

        <p class="text-sm font-medium text-teal-700 mb-4">
          {stageLabel(progressStage)}
        </p>

        <!-- Progress bar -->
        <div class="w-full bg-stone-200 rounded-full h-2 mb-2">
          <div
            class="bg-teal-600 h-2 rounded-full transition-all duration-300"
            style="width: {progressPct}%"
          ></div>
        </div>
        <p class="text-xs text-stone-400">{progressPct}%</p>

        <p class="text-xs text-stone-400 mt-6">
          AI analysis may take a minute per document.
        </p>
      </div>

    {:else if screen === 'success'}
      <!-- Success results -->
      <div class="w-full max-w-md">
        <div class="text-center mb-6">
          <div class="w-16 h-16 bg-green-50 rounded-2xl flex items-center justify-center mx-auto mb-4">
            <span class="text-3xl text-green-600">&#x2713;</span>
          </div>
          <h2 class="text-xl font-semibold text-stone-800 mb-1">
            {successCount === 1 ? 'Document ready for review' : `${successCount} documents ready for review`}
          </h2>
          {#if failureCount > 0}
            <p class="text-sm text-amber-600">
              {failureCount} file{failureCount === 1 ? '' : 's'} could not be processed.
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
                  Review
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
            Review document
          </button>
        {/if}

        <button
          class="w-full px-6 py-4 bg-white border border-stone-200 rounded-xl
                 text-base text-stone-700 min-h-[44px] mb-3"
          onclick={reset}
        >
          Import more
        </button>

        <button
          class="w-full text-sm text-stone-500 min-h-[44px]"
          onclick={() => navigation.navigate('home')}
        >
          Back to home
        </button>
      </div>

    {:else if screen === 'error'}
      <!-- Error state -->
      <div class="w-full max-w-md text-center">
        <div class="w-16 h-16 bg-red-50 rounded-2xl flex items-center justify-center mx-auto mb-4">
          <span class="text-3xl text-red-500">!</span>
        </div>
        <h2 class="text-lg font-semibold text-stone-800 mb-2">Something went wrong</h2>
        <p class="text-sm text-red-600 mb-6">{errorMessage}</p>

        <button
          class="w-full px-6 py-4 bg-[var(--color-primary)] text-white rounded-xl
                 text-base font-medium min-h-[44px] mb-3"
          onclick={reset}
        >
          Try again
        </button>
        <button
          class="w-full text-sm text-stone-500 min-h-[44px]"
          onclick={() => navigation.navigate('home')}
        >
          Back to home
        </button>
      </div>
    {/if}
  </div>
</div>
