<!--
  BTL-10 C7: DocumentListScreen v2 — Unified Documents screen.
  Integrates: ImportDropZone, ImportQueueSection, extended filter pills,
  delete modal, retry action, droppedFiles prop.
  Layout: Header → Drop zone → Queue section → Filter pills → Document list.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import { getDocuments } from '$lib/api/documents';
  import { deleteDocument, reprocessDocument } from '$lib/api/import';
  import type { DocumentCard } from '$lib/types/home';
  import type { DocumentLifecycleStatus } from '$lib/types/home';
  import DocumentCardView from '$lib/components/home/DocumentCardView.svelte';
  import DocumentSearch from '$lib/components/documents/DocumentSearch.svelte';
  import ImportDropZone from '$lib/components/documents/ImportDropZone.svelte';
  import ImportQueueSection from '$lib/components/documents/ImportQueueSection.svelte';
  import DeleteConfirmModal from '$lib/components/documents/DeleteConfirmModal.svelte';
  import { importQueue } from '$lib/stores/importQueue.svelte';
  import { navigation } from '$lib/stores/navigation.svelte';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import ErrorState from '$lib/components/ui/ErrorState.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import EmptyStateUI from '$lib/components/ui/EmptyState.svelte';
  import { DocsIcon, SearchIcon } from '$lib/components/icons/md';

  interface Props {
    /** File paths dropped from DropZoneOverlay. */
    droppedFiles?: string[];
  }
  let { droppedFiles }: Props = $props();

  let documents: DocumentCard[] = $state([]);
  let loading = $state(true);
  let loadingMore = $state(false);
  let hasMore = $state(true);
  let error: string | null = $state(null);
  let filterStatus = $state<'all' | 'pending' | 'confirmed' | 'failed'>('all');
  let showSearch = $state(false);

  // Delete modal state
  let deleteTarget: DocumentCard | null = $state(null);
  let deleting = $state(false);

  const PAGE_SIZE = 20;

  const PROCESSING_STATES: DocumentLifecycleStatus[] = ['Imported', 'Extracting', 'Structuring'];

  // -- Derived counts --

  // Filter processing docs from list — they are displayed in ImportQueueSection
  let visibleDocuments = $derived(
    importQueue.hasActiveImports
      ? documents.filter((d) => !PROCESSING_STATES.includes(d.status))
      : documents
  );
  let pendingCount = $derived(documents.filter((d) => d.status === 'PendingReview').length);
  let confirmedCount = $derived(documents.filter((d) => d.status === 'Confirmed').length);
  let failedCount = $derived(documents.filter((d) => d.status === 'Failed' || d.status === 'Rejected').length);

  // -- Filtered list --

  let filtered = $derived.by(() => {
    return visibleDocuments.filter((d) => {
      switch (filterStatus) {
        case 'pending':
          return d.status === 'PendingReview';
        case 'confirmed':
          return d.status === 'Confirmed';
        case 'failed':
          return d.status === 'Failed' || d.status === 'Rejected';
        default:
          return true;
      }
    });
  });

  // -- Data loading --

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

  // -- Actions --

  function handleDocumentTap(card: DocumentCard) {
    if (card.status === 'PendingReview') {
      navigation.navigate('review', { documentId: card.id });
    } else if (card.status === 'Failed' || card.status === 'Rejected') {
      // Failed/rejected cards don't navigate — actions are inline
      return;
    } else {
      navigation.navigate('document-detail', { documentId: card.id });
    }
  }

  function handleDeleteRequest(card: DocumentCard) {
    deleteTarget = card;
  }

  async function confirmDelete() {
    if (!deleteTarget) return;
    deleting = true;
    try {
      await deleteDocument(deleteTarget.id);
      documents = documents.filter((d) => d.id !== deleteTarget!.id);
      deleteTarget = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      deleting = false;
    }
  }

  async function handleRetry(card: DocumentCard) {
    try {
      await reprocessDocument(card.id);
      await loadDocuments();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  // -- Refresh on queue completion (new documents appear) --
  // Track which completed job IDs we've already reacted to, to avoid infinite loops.
  let seenCompletedIds = new Set<string>();

  $effect(() => {
    const completed = importQueue.completedItems;
    const newIds = completed.filter((j) => !seenCompletedIds.has(j.id));
    if (newIds.length > 0 && !loading) {
      for (const j of newIds) seenCompletedIds.add(j.id);
      loadDocuments();
    }
  });

  onMount(() => {
    loadDocuments();
    // importQueue listener is started at app level (+page.svelte), not here.
    importQueue.refresh();
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

    <!-- Filter pills: visible when documents exist -->
    {#if documents.length > 0}
      <div class="flex gap-2 overflow-x-auto pb-1">
        <!-- All -->
        <button
          class="shrink-0 px-3 py-1.5 rounded-full text-xs font-medium min-h-[32px]
                 {filterStatus === 'all'
                   ? 'bg-stone-800 dark:bg-gray-100 text-white dark:text-gray-900'
                   : 'bg-stone-100 dark:bg-gray-800 text-stone-600 dark:text-gray-300'}"
          onclick={() => { filterStatus = 'all'; }}
        >
          {$t('documents.list_filter_all', { values: { count: visibleDocuments.length } })}
        </button>

        <!-- Pending Review -->
        {#if pendingCount > 0}
          <button
            class="shrink-0 px-3 py-1.5 rounded-full text-xs font-medium min-h-[32px]
                   {filterStatus === 'pending'
                     ? 'bg-[var(--color-warning)] text-white'
                     : 'bg-[var(--color-warning-50)] text-[var(--color-warning-800)]'}"
            onclick={() => { filterStatus = filterStatus === 'pending' ? 'all' : 'pending'; }}
          >
            {$t('documents.list_filter_pending', { values: { count: pendingCount } })}
          </button>
        {/if}

        <!-- Confirmed -->
        {#if confirmedCount > 0}
          <button
            class="shrink-0 px-3 py-1.5 rounded-full text-xs font-medium min-h-[32px]
                   {filterStatus === 'confirmed'
                     ? 'bg-[var(--color-success)] text-white'
                     : 'bg-[var(--color-success-50)] text-[var(--color-success-800)]'}"
            onclick={() => { filterStatus = filterStatus === 'confirmed' ? 'all' : 'confirmed'; }}
          >
            {$t('documents.list_filter_confirmed', { values: { count: confirmedCount } })}
          </button>
        {/if}

        <!-- Failed -->
        {#if failedCount > 0}
          <button
            class="shrink-0 px-3 py-1.5 rounded-full text-xs font-medium min-h-[32px]
                   {filterStatus === 'failed'
                     ? 'bg-red-600 text-white'
                     : 'bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300'}"
            onclick={() => { filterStatus = filterStatus === 'failed' ? 'all' : 'failed'; }}
          >
            {$t('documents.list_filter_failed', { values: { count: failedCount } })}
          </button>
        {/if}
      </div>
    {/if}
  </header>

  <!-- Import drop zone -->
  <ImportDropZone hasDocuments={documents.length > 0} {droppedFiles} />

  <!-- Import queue section (active + failed jobs) -->
  <ImportQueueSection />

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

    {:else if documents.length === 0 && !importQueue.hasActiveImports}
      <EmptyStateUI
        icon={DocsIcon}
        title={$t('documents.list_empty_heading')}
        description={$t('documents.list_empty_description')}
      />

    {:else if filtered.length === 0}
      <div class="flex flex-col items-center justify-center py-12 text-center">
        <p class="text-stone-500 dark:text-gray-400 text-sm">{$t('documents.list_no_match')}</p>
      </div>

    {:else}
      <div class="space-y-3" role="list" aria-label={$t('documents.list_heading')}>
        {#each filtered as card (card.id)}
          <div role="listitem">
            <DocumentCardView
              {card}
              onTap={handleDocumentTap}
              onDelete={handleDeleteRequest}
              onRetry={handleRetry}
            />
          </div>
        {/each}
      </div>

      {#if hasMore && filterStatus === 'all'}
        <div class="flex justify-center py-6">
          <Button variant="ghost" loading={loadingMore} onclick={loadMore}>
            {loadingMore ? $t('documents.list_loading_more') : $t('documents.list_load_more')}
          </Button>
        </div>
      {/if}
    {/if}
  </div>
</div>

<!-- Delete confirmation modal -->
<DeleteConfirmModal
  open={deleteTarget !== null}
  filename={deleteTarget?.source_filename ?? deleteTarget?.document_type ?? ''}
  loading={deleting}
  onconfirm={confirmDelete}
  onclose={() => { deleteTarget = null; }}
/>
