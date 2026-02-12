<script lang="ts">
  import { onMount } from 'svelte';
  import { getHomeData, getMoreDocuments } from '$lib/api/home';
  import { listen } from '@tauri-apps/api/event';
  import type { HomeData, DocumentCard } from '$lib/types/home';
  import QuickActions from './QuickActions.svelte';
  import DocumentCardView from './DocumentCardView.svelte';
  import OnboardingMilestones from './OnboardingMilestones.svelte';
  import EmptyState from './EmptyState.svelte';

  interface Props {
    profileName: string;
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { profileName, onNavigate }: Props = $props();

  let homeData: HomeData | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);
  let loadingMore = $state(false);

  async function refresh() {
    try {
      loading = true;
      error = null;
      homeData = await getHomeData();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function loadMore() {
    if (!homeData || loadingMore) return;
    loadingMore = true;
    try {
      const more = await getMoreDocuments(homeData.recent_documents.length, 20);
      homeData.recent_documents = [...homeData.recent_documents, ...more];
    } catch (e) {
      console.error('Failed to load more documents:', e);
    } finally {
      loadingMore = false;
    }
  }

  function handleDocumentTap(card: DocumentCard) {
    if (card.status === 'PendingReview') {
      onNavigate('review', { documentId: card.id });
    } else {
      onNavigate('document-detail', { documentId: card.id });
    }
  }

  onMount(() => {
    refresh();
    const unlisten = listen('document-imported', () => refresh());
    return () => { unlisten.then(fn => fn()); };
  });

  let greeting = $derived(`Welcome back, ${profileName}`);

  function relativeTime(dateStr: string | null): string {
    if (!dateStr) return 'No documents yet';
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    if (diffMins < 1) return 'Just now';
    if (diffMins < 60) return `${diffMins} minute${diffMins === 1 ? '' : 's'} ago`;
    const diffHours = Math.floor(diffMins / 60);
    if (diffHours < 24) return `${diffHours} hour${diffHours === 1 ? '' : 's'} ago`;
    const diffDays = Math.floor(diffHours / 24);
    if (diffDays < 7) return `${diffDays} day${diffDays === 1 ? '' : 's'} ago`;
    return date.toLocaleDateString();
  }
</script>

<div class="flex flex-col min-h-screen pb-20 bg-stone-50">
  <!-- Header -->
  <header class="px-6 pt-6 pb-4">
    <h1 class="text-2xl font-bold text-stone-800">{greeting}</h1>
    {#if homeData}
      <p class="text-sm text-stone-500 mt-1">
        {homeData.stats.total_documents} document{homeData.stats.total_documents === 1 ? '' : 's'}
        · Last updated {relativeTime(homeData.stats.last_document_date)}
      </p>
    {/if}
  </header>

  {#if loading}
    <div class="flex items-center justify-center flex-1">
      <div class="animate-pulse text-stone-400">Loading...</div>
    </div>
  {:else if error}
    <div class="px-6 py-8 text-center">
      <p class="text-red-600 mb-4">Something went wrong: {error}</p>
      <button class="px-6 py-3 bg-stone-200 rounded-xl text-stone-700 min-h-[44px]"
              onclick={refresh}>
        Try again
      </button>
    </div>
  {:else if homeData}
    <!-- Quick actions -->
    <QuickActions
      hasDocuments={homeData.stats.total_documents > 0}
      {onNavigate}
    />

    <!-- Document feed or empty state -->
    {#if homeData.stats.total_documents === 0}
      <EmptyState {onNavigate} />
    {:else}
      <div class="px-6 py-3 flex flex-col gap-3">
        {#each homeData.recent_documents as card (card.id)}
          <DocumentCardView {card} onTap={handleDocumentTap} />
        {/each}

        {#if homeData.recent_documents.length < homeData.stats.total_documents}
          <button
            class="w-full py-3 text-sm text-teal-600 font-medium rounded-xl
                   bg-white border border-stone-200 hover:bg-stone-50 min-h-[44px]"
            onclick={loadMore}
            disabled={loadingMore}
          >
            {loadingMore ? 'Loading...' : 'Load more documents'}
          </button>
        {/if}
      </div>
    {/if}

    <!-- Onboarding milestones (new users) -->
    {#if !homeData.onboarding.first_document_loaded || !homeData.onboarding.first_question_asked}
      <OnboardingMilestones
        progress={homeData.onboarding}
        {onNavigate}
      />
    {/if}
  {/if}
</div>
