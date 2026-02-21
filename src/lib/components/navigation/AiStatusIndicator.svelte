<!-- V8-B2: Inline AI status indicator — replaces green dot + repeating toast -->
<!-- LP-01: Batch extraction progress | LP-06: Pending extraction count in header -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { ai } from '$lib/stores/ai.svelte';
  import { extraction } from '$lib/stores/extraction.svelte';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { ChevronDownIcon } from '$lib/components/icons/md';

  let open = $state(false);
  let containerEl: HTMLElement | undefined = $state();

  type StatusState = 'running' | 'fallback' | 'offline';

  let status: StatusState = $derived.by(() => {
    if (!ai.isAiAvailable) return 'offline';
    if (ai.activeModel?.source === 'Fallback') return 'fallback';
    return 'running';
  });

  const dotColors: Record<StatusState, string> = {
    running: 'bg-green-500',
    fallback: 'bg-amber-500',
    offline: 'bg-stone-400 dark:bg-gray-500',
  };

  let statusLabel = $derived.by(() => {
    if (status === 'offline') return $t('ai.status_offline');
    if (status === 'fallback') return $t('ai.status_fallback');
    return $t('ai.status_running');
  });

  let shortModelName = $derived.by(() => {
    if (!ai.activeModel?.name) return '';
    const name = ai.activeModel.name;
    // Strip org prefix: "MedAIBase/MedGemma1.5:4b" → "MedGemma1.5 4b"
    const afterSlash = name.includes('/') ? name.split('/').pop()! : name;
    // Strip tag: "medgemma:latest" → "MedGemma"
    return afterSlash.replace(':', ' ').replace('latest', '').trim();
  });

  let detailText = $derived.by(() => {
    if (status === 'offline') return $t('ai.status_detail_offline');
    if (status === 'fallback') return $t('ai.status_detail_fallback');
    return '';
  });

  /** LP-01: Batch progress display text. */
  let batchText = $derived.by(() => {
    if (!extraction.batch.running) return '';
    const { completed, total } = extraction.batch;
    if (total === 0) return $t('extraction.batch_starting');
    return $t('extraction.batch_progress', { values: { completed, total } });
  });

  function toggle() {
    open = !open;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && open) {
      open = false;
      (e.target as HTMLElement)?.focus();
    }
  }

  function handleClickOutside(e: MouseEvent) {
    if (open && containerEl && !containerEl.contains(e.target as Node)) {
      open = false;
    }
  }

  $effect(() => {
    if (open) {
      document.addEventListener('click', handleClickOutside, true);
      return () => document.removeEventListener('click', handleClickOutside, true);
    }
  });
</script>

<!-- Only render when AI store has initialized (not unknown) -->
{#if ai.statusLevel !== 'unknown' || ai.isAiAvailable}
  <div class="relative" bind:this={containerEl}>
    <button
      class="flex items-center gap-1.5 px-2 py-1 rounded-lg text-xs font-medium min-h-[32px]
             text-stone-600 dark:text-gray-400
             hover:bg-stone-100 dark:hover:bg-gray-800 transition-colors cursor-pointer"
      onclick={toggle}
      onkeydown={handleKeydown}
      aria-label="{statusLabel}{shortModelName ? ': ' + shortModelName : ''}"
      aria-haspopup="true"
      aria-expanded={open}
    >
      {#if extraction.batch.running}
        <!-- Pulsing dot during batch extraction -->
        <span class="w-2 h-2 rounded-full bg-[var(--color-primary)] animate-pulse flex-shrink-0" aria-hidden="true"></span>
      {:else}
        <span class="w-2 h-2 rounded-full {dotColors[status]} flex-shrink-0" aria-hidden="true"></span>
      {/if}
      {#if extraction.batch.running}
        <span class="hidden sm:inline text-[var(--color-primary)]">{batchText}</span>
      {:else if shortModelName}
        <span class="hidden sm:inline">{shortModelName}</span>
      {/if}
      {#if !extraction.batch.running && extraction.count > 0}
        <span
          class="text-[10px] font-bold min-w-[16px] h-[16px] px-1 rounded-full flex items-center justify-center
                 bg-[var(--color-primary)] text-white"
          aria-label={$t('extraction.badge_count', { values: { count: extraction.count } })}
        >
          {extraction.count > 99 ? '99+' : extraction.count}
        </span>
      {/if}
      <ChevronDownIcon class="w-3 h-3 transition-transform {open ? 'rotate-180' : ''}" />
    </button>

    {#if open}
      <div
        class="absolute right-0 top-full mt-1 w-64 bg-white dark:bg-gray-900 border border-stone-200 dark:border-gray-700
               rounded-xl shadow-lg p-4 z-50"
        role="tooltip"
      >
        <div class="flex items-center gap-2 mb-2">
          <span class="w-2.5 h-2.5 rounded-full {extraction.batch.running ? 'bg-[var(--color-primary)] animate-pulse' : dotColors[status]}" aria-hidden="true"></span>
          <span class="text-sm font-medium text-stone-800 dark:text-gray-100">{statusLabel}</span>
        </div>

        {#if ai.activeModel}
          <p class="text-xs text-stone-500 dark:text-gray-400 mb-1">
            {ai.activeModel.name}
          </p>
        {/if}

        {#if extraction.batch.running}
          <div class="mt-2 pt-2 border-t border-stone-100 dark:border-gray-800">
            <p class="text-xs font-medium text-[var(--color-primary)] mb-1">
              {batchText}
            </p>
            {#if extraction.batch.total > 0}
              <div class="w-full h-1.5 rounded-full bg-stone-100 dark:bg-gray-800 overflow-hidden">
                <div
                  class="h-full rounded-full bg-[var(--color-primary)] transition-[width] duration-300"
                  style="width: {Math.round((extraction.batch.completed / extraction.batch.total) * 100)}%"
                ></div>
              </div>
            {/if}
          </div>
        {:else if detailText}
          <p class="text-xs text-stone-500 dark:text-gray-400 leading-relaxed">
            {detailText}
          </p>
        {/if}

        {#if !extraction.batch.running && extraction.count > 0}
          <div class="mt-2 pt-2 border-t border-stone-100 dark:border-gray-800 flex items-center justify-between">
            <p class="text-xs text-stone-600 dark:text-gray-300">
              {$t('extraction.badge_count', { values: { count: extraction.count } })}
            </p>
            <button
              class="text-xs font-medium text-[var(--color-interactive)] hover:underline cursor-pointer"
              onclick={() => { navigation.navigate('home'); open = false; }}
            >
              {$t('settings.extraction_view') ?? 'Review'}
            </button>
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/if}
