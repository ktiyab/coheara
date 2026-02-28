<!--
  BTL-10 C6: ImportQueueCard — Single import job card with progress bar + cancel/retry.
  iOS Mail/Spark 2-line pattern for active jobs:
    ○ scan_certificat.png           Extracting...
      ████████████░░░░░░░░░  65%
-->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { ImportQueueItem } from '$lib/types/import-queue';
  import { ChevronDownIcon, CheckIcon, CloseIcon, RefreshIcon } from '$lib/components/icons/md';
  import { formatElapsed } from '$lib/utils/elapsed-time';

  interface Props {
    job: ImportQueueItem;
    queuePosition?: number;
    queueTotal?: number;
    onCancel?: (jobId: string) => void;
    onRetry?: (jobId: string) => void;
    onDelete?: (jobId: string) => void;
  }

  let { job, queuePosition, queueTotal, onCancel, onRetry, onDelete }: Props = $props();

  // Collapsible drawer — auto-expand for Failed, collapsed for others
  let detailExpanded = $state(false);

  $effect(() => {
    if (job.state === 'Failed') detailExpanded = true;
  });

  let hasDrawerContent = $derived(
    !['Done', 'Cancelled'].includes(job.state)
  );

  let stageLabel = $derived.by(() => {
    switch (job.state) {
      case 'Queued':
        return $t('documents.queue_queued');
      case 'Importing':
        return $t('import.stage_importing');
      case 'Extracting':
        return $t('import.stage_extracting');
      case 'Structuring':
        return $t('import.stage_structuring');
      case 'PendingReview':
        return $t('documents.queue_pending_review');
      case 'Done':
        return $t('import.stage_complete');
      case 'Failed':
        return $t('import.stage_failed');
      case 'Cancelled':
        return $t('documents.queue_cancelled');
      default:
        return job.state;
    }
  });

  let isActive = $derived(
    !['Done', 'Failed', 'Cancelled'].includes(job.state),
  );

  let isFailed = $derived(job.state === 'Failed');

  // -- Pipeline stepper --
  const STATE_ORDER: Record<string, number> = {
    Queued: -1, Importing: 0, Extracting: 1, Structuring: 2, PendingReview: 3, Done: 4,
  };

  type StepStatus = 'done' | 'active' | 'pending' | 'failed';

  function inferFailedStep(): number {
    if (job.progress_pct >= 60) return 2;
    if (job.progress_pct >= 15) return 1;
    return 0;
  }

  let steps = $derived.by((): { status: StepStatus; label: string }[] => {
    const failIdx = isFailed ? inferFailedStep() : -1;
    const curIdx = STATE_ORDER[job.state] ?? -1;

    const labelKeys = [
      { active: 'import.stage_importing', done: 'import.step_imported', failed: 'import.step_import_failed' },
      { active: 'import.stage_extracting', done: 'import.step_extracted', failed: 'import.step_extraction_failed' },
      { active: 'import.stage_structuring', done: 'import.step_structured', failed: 'import.step_structuring_failed' },
      { active: 'import.stage_saving', done: 'import.step_reviewed', failed: 'import.stage_failed' },
    ];

    return labelKeys.map((keys, i) => {
      let status: StepStatus;
      if (isFailed) {
        status = i < failIdx ? 'done' : i === failIdx ? 'failed' : 'pending';
      } else {
        status = i < curIdx ? 'done' : i === curIdx ? 'active' : 'pending';
      }
      const label = status === 'done' ? $t(keys.done)
        : status === 'failed' ? $t(keys.failed)
        : $t(keys.active);
      return { status, label };
    });
  });

  let statusDotClass = $derived.by(() => {
    if (isFailed) return 'text-red-500';
    if (job.state === 'Cancelled') return 'text-stone-400 dark:text-gray-500';
    if (job.state === 'Done') return 'text-[var(--color-success)]';
    return 'text-blue-500';
  });

  // Live elapsed time — ticks every second for active jobs, auto-stops when done
  let now = $state(Date.now());

  $effect(() => {
    if (!isActive) return;
    const timer = setInterval(() => { now = Date.now(); }, 1000);
    return () => clearInterval(timer);
  });

  let elapsedLabel = $derived.by(() => {
    if (job.state === 'Queued' && job.queued_at) {
      const secs = Math.floor((now - new Date(job.queued_at).getTime()) / 1000);
      return formatElapsed(Math.max(0, secs));
    }
    if (isActive && job.state !== 'Queued' && job.started_at) {
      const secs = Math.floor((now - new Date(job.started_at).getTime()) / 1000);
      return formatElapsed(Math.max(0, secs));
    }
    return null;
  });
</script>

<div
  class="w-full bg-white dark:bg-gray-900 rounded-xl p-4 shadow-sm border border-stone-100 dark:border-gray-800"
  role="listitem"
>
  <!-- Line 1: status dot + filename + stage label + cancel -->
  <div class="flex items-center gap-2">
    <!-- Status dot -->
    <span class="shrink-0 text-lg leading-none {statusDotClass}" aria-hidden="true">
      {#if isFailed}&#x2715;{:else if job.state === 'Done'}&#x25CF;{:else}&#x25CB;{/if}
    </span>

    <!-- Filename (bold, truncated) -->
    <span class="flex-1 min-w-0 font-medium text-sm text-stone-800 dark:text-gray-100 truncate">
      {job.filename}
    </span>

    <!-- Stage label (right-aligned) -->
    <span
      class="shrink-0 text-xs {isFailed ? 'text-red-500 font-medium' : 'text-stone-500 dark:text-gray-400'}"
    >
      {stageLabel}
    </span>

    <!-- Elapsed time -->
    {#if elapsedLabel}
      <span class="shrink-0 text-xs text-stone-400 dark:text-gray-500 tabular-nums">
        {elapsedLabel}
      </span>
    {/if}

    <!-- Chevron toggle for detail drawer -->
    {#if hasDrawerContent}
      <button
        class="shrink-0 min-h-[32px] min-w-[32px] flex items-center justify-center
               text-stone-400 dark:text-gray-500 hover:text-stone-600 dark:hover:text-gray-300 transition-colors"
        onclick={() => { detailExpanded = !detailExpanded; }}
        aria-expanded={detailExpanded}
        aria-label="Toggle details"
      >
        <ChevronDownIcon class="w-4 h-4 transition-transform {detailExpanded ? 'rotate-180' : ''}" />
      </button>
    {/if}
  </div>

  <!-- Line 2: progress bar (active non-queued), queue position (queued), or error + actions (failed) -->
  {#if isActive && job.state !== 'Queued'}
    <div class="mt-2 flex items-center gap-2 pl-6">
      <div class="flex-1 h-1.5 bg-stone-200 dark:bg-gray-700 rounded-full overflow-hidden">
        <div
          class="h-full bg-blue-500 rounded-full transition-all duration-300"
          style="width: {job.progress_pct}%"
        ></div>
      </div>
      <span class="shrink-0 text-xs text-stone-500 dark:text-gray-400 tabular-nums">
        {job.progress_pct}%
      </span>
    </div>
  {:else if job.state === 'Queued' && queuePosition && queueTotal}
    <div class="mt-1 pl-6">
      <span class="text-xs text-stone-400 dark:text-gray-500">
        {$t('documents.queue_position', { values: { position: queuePosition, total: queueTotal } })}
      </span>
    </div>
  {/if}

  <!-- Collapsible detail drawer — pipeline stepper -->
  {#if hasDrawerContent && detailExpanded}
    <div class="mt-3 pl-6 space-y-3">
      <!-- Vertical pipeline stepper -->
      <div class="flex flex-col" role="list" aria-label="Pipeline progress">
        {#each steps as step, i}
          <div class="flex items-start gap-2.5" role="listitem">
            <div class="flex flex-col items-center">
              {#if step.status === 'done'}
                <span class="w-5 h-5 rounded-full bg-[var(--color-success)] flex items-center justify-center shrink-0">
                  <CheckIcon class="w-3 h-3 text-white" />
                </span>
              {:else if step.status === 'active'}
                <span class="w-5 h-5 rounded-full bg-blue-500 flex items-center justify-center shrink-0">
                  <span class="w-2 h-2 rounded-full bg-white animate-pulse"></span>
                </span>
              {:else if step.status === 'failed'}
                <span class="w-5 h-5 rounded-full bg-red-500 flex items-center justify-center shrink-0">
                  <span class="text-white text-xs font-bold leading-none">&#x2715;</span>
                </span>
              {:else}
                <span class="w-5 h-5 rounded-full border-2 border-stone-300 dark:border-gray-600 shrink-0"></span>
              {/if}
              {#if i < steps.length - 1}
                <div class="w-0.5 h-4 {step.status === 'done' ? 'bg-[var(--color-success)]' : 'bg-stone-200 dark:bg-gray-700'}"></div>
              {/if}
            </div>
            <span class="text-xs leading-5 {
              step.status === 'done' ? 'text-stone-500 dark:text-gray-400'
              : step.status === 'active' ? 'text-stone-800 dark:text-gray-100 font-medium'
              : step.status === 'failed' ? 'text-red-500 font-medium'
              : 'text-stone-400 dark:text-gray-500'
            }">
              {step.label}
            </span>
          </div>
        {/each}
      </div>

      <!-- Error message -->
      {#if isFailed && job.error}
        <p class="text-xs text-red-500 dark:text-red-400 line-clamp-3">{job.error}</p>
      {/if}

      <!-- Metadata -->
      <div class="space-y-1">
        {#if job.model_used}
          <p class="text-xs text-stone-500 dark:text-gray-400">
            <span class="text-stone-400 dark:text-gray-500">{$t('documents.queue_detail_model')}:</span> {job.model_used}
          </p>
        {/if}
        <p class="text-xs text-stone-500 dark:text-gray-400 truncate">
          <span class="text-stone-400 dark:text-gray-500">{$t('documents.queue_detail_file')}:</span> {job.file_path}
        </p>
      </div>

      <!-- Action buttons -->
      <div class="flex items-center gap-2">
        {#if isFailed}
          {#if onRetry}
            <button
              class="inline-flex items-center gap-1 px-2.5 py-1 min-h-[32px] text-xs font-medium
                     text-[var(--color-success)] hover:bg-[var(--color-success-50)] dark:hover:bg-[var(--color-success-900)]/10
                     rounded-lg transition-colors"
              onclick={() => onRetry(job.id)}
            >
              <RefreshIcon class="w-3.5 h-3.5" /> {$t('common.retry')}
            </button>
          {/if}
          {#if onDelete}
            <button
              class="inline-flex items-center gap-1 px-2.5 py-1 min-h-[32px] text-xs font-medium
                     text-red-600 hover:bg-red-50 dark:hover:bg-red-900/10
                     rounded-lg transition-colors"
              onclick={() => onDelete(job.id)}
            >
              {$t('documents.queue_delete')}
            </button>
          {/if}
        {:else if isActive && onCancel}
          <button
            class="inline-flex items-center gap-1 px-2.5 py-1 min-h-[32px] text-xs font-medium
                   text-stone-500 dark:text-gray-400 hover:text-red-600 hover:bg-red-50 dark:hover:bg-red-900/10
                   rounded-lg transition-colors"
            onclick={() => onCancel(job.id)}
          >
            <CloseIcon class="w-3.5 h-3.5" /> {$t('common.cancel')}
          </button>
        {/if}
      </div>
    </div>
  {/if}
</div>
