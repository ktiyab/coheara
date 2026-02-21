<!-- Spec 46 [CG-06] + Spec 49: Full-text document search with debounce. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { searchDocuments, type DocumentSearchResult } from '$lib/api/documents';
  import { SearchIcon, CloseIcon } from '$lib/components/icons/md';

  interface Props {
    onSelect: (documentId: string) => void;
    onClose: () => void;
  }

  let { onSelect, onClose }: Props = $props();

  let query = $state('');
  let results: DocumentSearchResult[] = $state([]);
  let searching = $state(false);
  let searched = $state(false);
  let debounceTimer: ReturnType<typeof setTimeout> | null = $state(null);

  function handleInput(value: string) {
    query = value;
    searched = false;

    if (debounceTimer) clearTimeout(debounceTimer);

    if (value.trim().length < 2) {
      results = [];
      searching = false;
      return;
    }

    searching = true;
    debounceTimer = setTimeout(() => doSearch(value), 300);
  }

  async function doSearch(q: string) {
    try {
      results = await searchDocuments(q);
    } catch {
      results = [];
    } finally {
      searching = false;
      searched = true;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      onClose();
    }
  }
</script>

<div class="flex flex-col gap-3">
  <!-- Search input -->
  <div class="relative">
    <span class="absolute left-3 top-1/2 -translate-y-1/2 text-[var(--color-text-muted)]" aria-hidden="true">
      <SearchIcon class="w-4 h-4" />
    </span>
    <input
      type="search"
      class="w-full pl-9 pr-9 py-2.5 rounded-lg border border-[var(--color-border)]
             bg-[var(--color-surface)] text-sm text-[var(--color-text-primary)]
             placeholder:text-[var(--color-text-muted)]
             focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)]"
      placeholder={$t('documents.search_placeholder')}
      aria-label={$t('documents.search_placeholder')}
      value={query}
      oninput={(e) => handleInput(e.currentTarget.value)}
      onkeydown={handleKeydown}
    />
    {#if query}
      <button
        class="absolute right-3 top-1/2 -translate-y-1/2 text-[var(--color-text-muted)]
               hover:text-[var(--color-text-primary)] text-sm"
        onclick={() => handleInput('')}
        aria-label={$t('documents.search_clear')}
      >
        <CloseIcon class="w-3.5 h-3.5" />
      </button>
    {/if}
  </div>

  <!-- Results -->
  {#if searching}
    <p class="text-sm text-[var(--color-text-muted)] text-center py-4">
      {$t('documents.search_searching')}
    </p>
  {:else if searched && results.length === 0}
    <p class="text-sm text-[var(--color-text-muted)] text-center py-4">
      {$t('documents.search_no_results')}
    </p>
  {:else if results.length > 0}
    <ul class="divide-y divide-[var(--color-border)]" role="listbox" aria-label={$t('documents.search_results')}>
      {#each results as result (result.document_id)}
        <li role="option" aria-selected="false">
          <button
            class="w-full text-left px-3 py-3 hover:bg-[var(--color-surface-hover)]
                   focus:bg-[var(--color-surface-hover)] focus:outline-none rounded-md transition-colors"
            onclick={() => onSelect(result.document_id)}
          >
            <p class="text-sm font-medium text-[var(--color-text-primary)] truncate">
              {result.title}
            </p>
            {#if result.professional_name}
              <p class="text-xs text-[var(--color-text-muted)] mt-0.5">
                {result.professional_name}
              </p>
            {/if}
            <p class="text-xs text-[var(--color-text-secondary)] mt-1 line-clamp-2">
              {@html result.snippet}
            </p>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
