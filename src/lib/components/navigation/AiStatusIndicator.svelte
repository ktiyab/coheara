<!-- V8-B2: Inline AI status indicator — replaces green dot + repeating toast -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { ai } from '$lib/stores/ai.svelte';
  import { ChevronDownOutline } from 'flowbite-svelte-icons';

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
      <span class="w-2 h-2 rounded-full {dotColors[status]} flex-shrink-0" aria-hidden="true"></span>
      {#if shortModelName}
        <span class="hidden sm:inline">{shortModelName}</span>
      {/if}
      <ChevronDownOutline class="w-3 h-3 transition-transform {open ? 'rotate-180' : ''}" />
    </button>

    {#if open}
      <div
        class="absolute right-0 top-full mt-1 w-64 bg-white dark:bg-gray-900 border border-stone-200 dark:border-gray-700
               rounded-xl shadow-lg p-4 z-50"
        role="tooltip"
      >
        <div class="flex items-center gap-2 mb-2">
          <span class="w-2.5 h-2.5 rounded-full {dotColors[status]}" aria-hidden="true"></span>
          <span class="text-sm font-medium text-stone-800 dark:text-gray-100">{statusLabel}</span>
        </div>

        {#if ai.activeModel}
          <p class="text-xs text-stone-500 dark:text-gray-400 mb-1">
            {ai.activeModel.name}
          </p>
        {/if}

        {#if detailText}
          <p class="text-xs text-stone-500 dark:text-gray-400 leading-relaxed">
            {detailText}
          </p>
        {/if}
      </div>
    {/if}
  </div>
{/if}
