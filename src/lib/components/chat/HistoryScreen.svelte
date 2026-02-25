<!-- UA02-06: Dedicated History view — conversation list in main content area.
     Pattern: Telegram/WhatsApp chat list. Full-width list with previews.
     Sibling to "Chat" (Ask) in navigation. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import { listConversations, deleteConversation } from '$lib/api/chat';
  import type { ConversationSummary } from '$lib/types/chat';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { SearchIcon, DeleteIcon } from '$lib/components/icons/md';
  import EmptyStateUI from '$lib/components/ui/EmptyState.svelte';
  import { HistoryIcon } from '$lib/components/icons/md';

  let conversations: ConversationSummary[] = $state([]);
  let searchQuery = $state('');
  let confirmDeleteId: string | null = $state(null);

  const PAGE_SIZE = 20;
  let displayCount = $state(PAGE_SIZE);

  let filtered = $derived(
    searchQuery.trim()
      ? conversations.filter(c =>
          c.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
          c.last_message_preview.toLowerCase().includes(searchQuery.toLowerCase())
        )
      : conversations
  );

  let displayed = $derived(filtered.slice(0, displayCount));
  let hasMore = $derived(displayCount < filtered.length);

  function loadMore() {
    displayCount += PAGE_SIZE;
  }

  // Reset pagination when search changes
  $effect(() => {
    searchQuery;
    displayCount = PAGE_SIZE;
  });

  async function loadConversations() {
    try {
      conversations = await listConversations();
    } catch (e) {
      console.error('Failed to load conversations:', e);
    }
  }

  async function handleDelete(id: string) {
    await deleteConversation(id);
    conversations = conversations.filter(c => c.id !== id);
    confirmDeleteId = null;
  }

  function openConversation(id: string) {
    navigation.navigate('chat', { conversationId: id });
  }

  function startNewConversation() {
    navigation.navigate('chat');
  }

  function relativeTime(dateStr: string): string {
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    if (diffMins < 1) return $t('chat.time_just_now');
    if (diffMins < 60) return $t('chat.time_minutes_ago', { values: { count: diffMins } });
    const diffHours = Math.floor(diffMins / 60);
    if (diffHours < 24) return $t('chat.time_hours_ago', { values: { count: diffHours } });
    const diffDays = Math.floor(diffHours / 24);
    if (diffDays < 7) return $t('chat.time_days_ago', { values: { count: diffDays } });
    return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
  }

  onMount(loadConversations);
</script>

<div class="flex flex-col h-full bg-stone-50 dark:bg-gray-950">
  <!-- Header -->
  <header class="flex items-center gap-3 px-[var(--spacing-page-x)] pt-6 pb-4">
    <h1 class="flex-1 text-2xl font-bold text-stone-800 dark:text-gray-100">{$t('nav.history')}</h1>
    <button
      class="min-h-[44px] min-w-[44px] px-4 py-2 flex items-center gap-2 rounded-xl
             bg-[var(--color-success)] text-white text-sm font-medium
             hover:opacity-90 transition-colors"
      onclick={startNewConversation}
      aria-label={$t('chat.new_conversation')}
    >
      <SearchIcon class="w-5 h-5" />
      <span>{$t('chat.new_conversation')}</span>
    </button>
  </header>

  <!-- Search bar -->
  {#if conversations.length > 0}
    <div class="px-[var(--spacing-page-x)] pb-3">
      <div class="relative">
        <SearchIcon class="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-stone-400 dark:text-gray-500" />
        <input
          type="text"
          bind:value={searchQuery}
          placeholder={$t('chat.search_conversations')}
          class="w-full pl-10 pr-4 py-2.5 rounded-xl border border-stone-200 dark:border-gray-700
                 bg-white dark:bg-gray-900 text-sm text-stone-800 dark:text-gray-100
                 placeholder:text-stone-400 dark:placeholder:text-gray-500
                 focus:border-[var(--color-success)] focus:outline-none min-h-[44px]"
        />
      </div>
    </div>
  {/if}

  <!-- Conversation list -->
  <div class="flex-1 overflow-y-auto px-[var(--spacing-page-x)] pb-6">
    {#if conversations.length === 0}
      <EmptyStateUI
        icon={HistoryIcon}
        title={$t('chat.no_conversations')}
        description={$t('chat.history_empty_description')}
      />
    {:else if filtered.length === 0}
      <div class="flex items-center justify-center py-16">
        <p class="text-sm text-stone-500 dark:text-gray-400">{$t('chat.no_search_results')}</p>
      </div>
    {:else}
      <div class="bg-white dark:bg-gray-900 rounded-[var(--radius-card)] border border-stone-100 dark:border-gray-800 shadow-sm divide-y divide-stone-100 dark:divide-gray-800">
        {#each displayed as conv (conv.id)}
          <div class="group relative">
            <button
              class="w-full text-left px-4 py-3.5 hover:bg-stone-50 dark:hover:bg-gray-800 transition-colors min-h-[68px]
                     first:rounded-t-[var(--radius-card)] last:rounded-b-[var(--radius-card)]"
              onclick={() => openConversation(conv.id)}
            >
              <div class="flex items-start gap-3">
                <div class="w-10 h-10 rounded-full bg-[var(--color-success)]/10 flex items-center justify-center flex-shrink-0 mt-0.5">
                  <SearchIcon class="w-5 h-5 text-[var(--color-success)]" />
                </div>
                <div class="flex-1 min-w-0">
                  <div class="flex items-baseline justify-between gap-2">
                    <p class="text-sm font-medium text-stone-800 dark:text-gray-100 truncate">{conv.title}</p>
                    <span class="text-xs text-stone-400 dark:text-gray-500 flex-shrink-0">{relativeTime(conv.last_message_at)}</span>
                  </div>
                  <p class="text-xs text-stone-500 dark:text-gray-400 truncate mt-1">{conv.last_message_preview || $t('chat.no_messages')}</p>
                  <p class="text-xs text-stone-400 dark:text-gray-500 mt-0.5">
                    {$t('chat.message_count', { values: { count: conv.message_count } })}
                  </p>
                </div>
              </div>
            </button>

            <!-- Delete button — visible on hover -->
            <button
              class="absolute right-3 top-1/2 -translate-y-1/2 min-h-[36px] min-w-[36px]
                     flex items-center justify-center rounded-lg
                     text-stone-400 dark:text-gray-500
                     hover:text-[var(--color-danger)] hover:bg-stone-100 dark:hover:bg-gray-800
                     opacity-0 group-hover:opacity-100 focus:opacity-100
                     transition-all"
              onclick={(e) => { e.stopPropagation(); confirmDeleteId = conv.id; }}
              aria-label={$t('common.delete')}
            >
              <DeleteIcon class="w-5 h-5" />
            </button>

            <!-- Confirm overlay -->
            {#if confirmDeleteId === conv.id}
              <div class="absolute inset-0 bg-white dark:bg-gray-900 flex items-center justify-between px-4 z-10
                          rounded-[var(--radius-card)]">
                <span class="text-sm text-stone-600 dark:text-gray-300">{$t('chat.delete_confirmation')}</span>
                <div class="flex gap-2">
                  <button
                    class="px-3 py-1.5 text-sm text-stone-500 dark:text-gray-400 min-h-[36px] rounded-lg
                           hover:bg-stone-100 dark:hover:bg-gray-800 transition-colors"
                    onclick={() => confirmDeleteId = null}
                  >
                    {$t('common.cancel')}
                  </button>
                  <button
                    class="px-3 py-1.5 text-sm text-white bg-[var(--color-danger)] font-medium min-h-[36px] rounded-lg
                           hover:opacity-90 transition-opacity"
                    onclick={() => handleDelete(conv.id)}
                  >
                    {$t('common.delete')}
                  </button>
                </div>
              </div>
            {/if}
          </div>
        {/each}
      </div>

      {#if hasMore}
        <div class="flex justify-center py-4">
          <button
            class="px-4 py-2 text-sm font-medium text-stone-600 dark:text-gray-400
                   hover:text-stone-800 dark:hover:text-gray-200 transition-colors"
            onclick={loadMore}
          >
            {$t('documents.list_load_more')}
          </button>
        </div>
      {/if}
    {/if}
  </div>
</div>
