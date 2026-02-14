<!-- E2E-F04: Full document list with search/filter and infinite scroll. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { getDocuments } from '$lib/api/documents';
  import type { DocumentCard } from '$lib/types/home';
  import DocumentCardView from '$lib/components/home/DocumentCardView.svelte';
  import { navigation } from '$lib/stores/navigation.svelte';

  let documents: DocumentCard[] = $state([]);
  let loading = $state(true);
  let loadingMore = $state(false);
  let hasMore = $state(true);
  let error: string | null = $state(null);
  let filterType = $state('all');
  let filterStatus = $state('all');

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

<div class="flex flex-col min-h-screen bg-stone-50 pb-20">
  <!-- Header -->
  <header class="px-4 py-4 bg-white border-b border-stone-200 shrink-0">
    <div class="flex items-center justify-between mb-3">
      <h1 class="text-xl font-semibold text-stone-800">Documents</h1>
      <button
        class="px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg text-sm
               font-medium min-h-[44px]"
        onclick={() => navigation.navigate('import')}
      >
        + Import
      </button>
    </div>

    <!-- Filters -->
    <div class="flex gap-2 overflow-x-auto pb-1">
      <button
        class="shrink-0 px-3 py-1.5 rounded-full text-xs font-medium min-h-[32px]
               {filterStatus === 'all' && filterType === 'all'
                 ? 'bg-stone-800 text-white'
                 : 'bg-stone-100 text-stone-600'}"
        onclick={() => { filterType = 'all'; filterStatus = 'all'; }}
      >
        All ({documents.length})
      </button>

      {#if pendingCount > 0}
        <button
          class="shrink-0 px-3 py-1.5 rounded-full text-xs font-medium min-h-[32px]
                 {filterStatus === 'pending'
                   ? 'bg-amber-600 text-white'
                   : 'bg-amber-50 text-amber-700'}"
          onclick={() => { filterStatus = filterStatus === 'pending' ? 'all' : 'pending'; filterType = 'all'; }}
        >
          Pending ({pendingCount})
        </button>
      {/if}

      {#each documentTypes as dtype}
        <button
          class="shrink-0 px-3 py-1.5 rounded-full text-xs font-medium min-h-[32px]
                 {filterType === dtype
                   ? 'bg-stone-800 text-white'
                   : 'bg-stone-100 text-stone-600'}"
          onclick={() => { filterType = filterType === dtype ? 'all' : dtype; filterStatus = 'all'; }}
        >
          {dtype}
        </button>
      {/each}
    </div>
  </header>

  <!-- Document list -->
  <div class="flex-1 px-4 py-4">
    {#if loading}
      <div class="flex items-center justify-center py-12">
        <div class="animate-pulse text-stone-400">Loading documents...</div>
      </div>

    {:else if error}
      <div class="flex flex-col items-center justify-center py-12 text-center">
        <p class="text-red-600 mb-4">Something went wrong: {error}</p>
        <button
          class="px-6 py-3 bg-stone-200 rounded-xl text-stone-700 min-h-[44px]"
          onclick={loadDocuments}
        >
          Try again
        </button>
      </div>

    {:else if documents.length === 0}
      <div class="flex flex-col items-center justify-center py-16 text-center">
        <div class="w-16 h-16 bg-stone-100 rounded-2xl flex items-center justify-center mb-4">
          <span class="text-2xl text-stone-400">&#128196;</span>
        </div>
        <h2 class="text-lg font-semibold text-stone-700 mb-2">No documents yet</h2>
        <p class="text-sm text-stone-500 mb-6">
          Import your first medical document to get started.
        </p>
        <button
          class="px-6 py-3 bg-[var(--color-primary)] text-white rounded-xl
                 text-sm font-medium min-h-[44px]"
          onclick={() => navigation.navigate('import')}
        >
          Import documents
        </button>
      </div>

    {:else if filtered.length === 0}
      <div class="flex flex-col items-center justify-center py-12 text-center">
        <p class="text-stone-500 text-sm">No documents match the current filter.</p>
      </div>

    {:else}
      <div class="space-y-3">
        {#each filtered as card (card.id)}
          <DocumentCardView {card} onTap={handleDocumentTap} />
        {/each}
      </div>

      {#if hasMore && filterType === 'all' && filterStatus === 'all'}
        <div class="flex justify-center py-6">
          <button
            class="px-6 py-3 bg-white border border-stone-200 rounded-xl text-sm
                   text-stone-600 min-h-[44px] disabled:opacity-50"
            disabled={loadingMore}
            onclick={loadMore}
          >
            {loadingMore ? 'Loading...' : 'Load more'}
          </button>
        </div>
      {/if}
    {/if}
  </div>
</div>
