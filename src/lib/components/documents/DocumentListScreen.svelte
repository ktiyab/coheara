<!-- E2E-F04: Full document list with search/filter and infinite scroll. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import { getDocuments } from '$lib/api/documents';
  import type { DocumentCard } from '$lib/types/home';
  import DocumentCardView from '$lib/components/home/DocumentCardView.svelte';
  import DocumentSearch from '$lib/components/documents/DocumentSearch.svelte';
  import { navigation } from '$lib/stores/navigation.svelte';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import ErrorState from '$lib/components/ui/ErrorState.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import EmptyStateUI from '$lib/components/ui/EmptyState.svelte';
  import { DocsIcon, SearchIcon } from '$lib/components/icons/md';

  let documents: DocumentCard[] = $state([]);
  let loading = $state(true);
  let loadingMore = $state(false);
  let hasMore = $state(true);
  let error: string | null = $state(null);
  let filterType = $state('all');
  let filterStatus = $state('all');
  let showSearch = $state(false);

  const PAGE_SIZE = 20;

  let filtered = $derived.by(() => {
    return documents.filter((d) => {
      if (filterType !== 'all' && d.document_type !== filterType) return false;
      if (filterStatus === 'pending' && d.status !== 'PendingReview') return false;
      if (filterStatus === 'confirmed' && d.status !== 'Confirmed') return false;
      return true;
    });
  });

  let documentTypes = $derived.by(() => {
    const types = new Set(documents.map((d) => d.document_type));
    return Array.from(types).sort();
  });

  let pendingCount = $derived(documents.filter((d) => d.status === 'PendingReview').length);

  async function loadDocuments() {
    loading = true;
    error = null;
    try {
      documents = await getDocuments(0, PAGE_SIZE);
      hasMore = documents.length >= PAGE_SIZE;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      documents = [];
    } finally {
      loading = false;
    }
  }

  async function loadMore() {
    if (loadingMore || !hasMore) return;
    loadingMore = true;
    try {
      const more = await getDocuments(documents.length, PAGE_SIZE);
      documents = [...documents, ...more];
      hasMore = more.length >= PAGE_SIZE;
    } catch {
      // Silently ignore
    } finally {
      loadingMore = false;
    }
  }

  function handleDocumentTap(card: DocumentCard) {
    if (card.status === 'PendingReview') {
      navigation.navigate('review', { documentId: card.id });
    } else {
      navigation.navigate('document-detail', { documentId: card.id });
    }
  }

  onMount(() => {
    loadDocuments();
  });
</script>

<div class="flex flex-col bg-stone-50 dark:bg-gray-950">
  <!-- Header -->
  <header class="px-4 py-4 bg-stone-50 dark:bg-gray-950 shrink-0">
    <div class="flex items-center justify-between {documents.length > 0 ? 'mb-3' : ''}">
      <h1 class="text-2xl font-bold text-stone-800 dark:text-gray-100">{$t('documents.list_heading')}</h1>
      {#if documents.length > 0}
        <div class="flex items-center gap-2">
          <button
            class="inline-flex items-center justify-center w-9 h-9 rounded-lg
                   bg-transparent text-stone-600 dark:text-gray-300 border border-stone-200 dark:border-gray-700
                   hover:bg-stone-50 dark:hover:bg-gray-800 active:bg-stone-100 dark:active:bg-gray-700 transition-colors
                   focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-primary)]"
            onclick={() => { showSearch = !showSearch; }}
            aria-label={$t('documents.search_toggle')}
            aria-expanded={showSearch}
          >
            <SearchIcon class="w-4 h-4" />
          </button>
          <Button variant="primary" size="sm" onclick={() => navigation.navigate('import')}>
            {$t('documents.list_import')}
          </Button>
        </div>
      {/if}
    </div>

    {#if showSearch && documents.length > 0}
      <div class="mb-3">
        <DocumentSearch
          onSelect={(docId) => {
            showSearch = false;
            navigation.navigate('document-detail', { documentId: docId });
          }}
          onClose={() => { showSearch = false; }}
        />
      </div>
    {/if}

    <!-- Filters: only visible when documents exist (V10-1, V10-5: S8-1) -->
    {#if documents.length > 0}
      <div class="flex gap-2 overflow-x-auto pb-1">
        <button
          class="shrink-0 px-3 py-1.5 rounded-full text-xs font-medium min-h-[32px]
                 {filterStatus === 'all' && filterType === 'all'
                   ? 'bg-stone-800 dark:bg-gray-100 text-white dark:text-gray-900'
                   : 'bg-stone-100 dark:bg-gray-800 text-stone-600 dark:text-gray-300'}"
          onclick={() => { filterType = 'all'; filterStatus = 'all'; }}
        >
          {$t('documents.list_filter_all', { values: { count: documents.length } })}
        </button>

        {#if pendingCount > 0}
          <button
            class="shrink-0 px-3 py-1.5 rounded-full text-xs font-medium min-h-[32px]
                   {filterStatus === 'pending'
                     ? 'bg-[var(--color-warning)] text-white'
                     : 'bg-[var(--color-warning-50)] text-[var(--color-warning-800)]'}"
            onclick={() => { filterStatus = filterStatus === 'pending' ? 'all' : 'pending'; filterType = 'all'; }}
          >
            {$t('documents.list_filter_pending', { values: { count: pendingCount } })}
          </button>
        {/if}

        {#each documentTypes as dtype}
          <button
            class="shrink-0 px-3 py-1.5 rounded-full text-xs font-medium min-h-[32px]
                   {filterType === dtype
                     ? 'bg-stone-800 dark:bg-gray-100 text-white dark:text-gray-900'
                     : 'bg-stone-100 dark:bg-gray-800 text-stone-600 dark:text-gray-300'}"
            onclick={() => { filterType = filterType === dtype ? 'all' : dtype; filterStatus = 'all'; }}
          >
            {dtype}
          </button>
        {/each}
      </div>
    {/if}
  </header>

  <!-- Document list -->
  <div class="flex-1 px-4 py-4">
    {#if loading}
      <LoadingState message={$t('documents.list_loading')} />

    {:else if error}
      <ErrorState
        message="{$t('documents.list_error_prefix')} {error}"
        onretry={loadDocuments}
        retryLabel={$t('documents.list_try_again')}
      />

    {:else if documents.length === 0}
      <EmptyStateUI
        icon={DocsIcon}
        title={$t('documents.list_empty_heading')}
        description={$t('documents.list_empty_description')}
        actionLabel={$t('documents.list_import_documents')}
        onaction={() => navigation.navigate('import')}
      />

    {:else if filtered.length === 0}
      <div class="flex flex-col items-center justify-center py-12 text-center">
        <p class="text-stone-500 dark:text-gray-400 text-sm">{$t('documents.list_no_match')}</p>
      </div>

    {:else}
      <div class="space-y-3" role="list" aria-label={$t('documents.list_heading')}>
        {#each filtered as card (card.id)}
          <div role="listitem">
            <DocumentCardView {card} onTap={handleDocumentTap} />
          </div>
        {/each}
      </div>

      {#if hasMore && filterType === 'all' && filterStatus === 'all'}
        <div class="flex justify-center py-6">
          <Button variant="ghost" loading={loadingMore} onclick={loadMore}>
            {loadingMore ? $t('documents.list_loading_more') : $t('documents.list_load_more')}
          </Button>
        </div>
      {/if}
    {/if}
  </div>
</div>
