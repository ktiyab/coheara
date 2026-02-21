<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { ConversationSummary } from '$lib/types/chat';
  import Button from '$lib/components/ui/Button.svelte';

  interface Props {
    conversations: ConversationSummary[];
    activeConversationId: string | null;
    onSelect: (id: string) => void;
    onNewConversation: () => void;
    onDelete: (id: string) => void;
  }
  let { conversations, activeConversationId, onSelect, onNewConversation, onDelete }: Props = $props();

  let confirmDeleteId: string | null = $state(null);

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
</script>

<div class="flex flex-col h-full">
  <div class="px-4 py-4 border-b border-stone-200 dark:border-gray-700">
    <h2 class="text-lg font-bold text-stone-800 dark:text-gray-100">{$t('chat.conversations_heading')}</h2>
    <div class="mt-2">
      <Button variant="primary" fullWidth onclick={onNewConversation}>
        {$t('chat.new_conversation_button')}
      </Button>
    </div>
  </div>

  <div class="flex-1 overflow-y-auto" role="list" aria-label={$t('chat.conversations_heading')}>
    {#if conversations.length === 0}
      <div class="px-4 py-8 text-center">
        <p class="text-sm text-stone-500 dark:text-gray-400">{$t('chat.no_conversations')}</p>
      </div>
    {:else}
      {#each conversations as conv (conv.id)}
        <div class="relative" role="listitem">
          <button
            class="w-full text-left px-4 py-3 border-b border-stone-100 dark:border-gray-800
                   hover:bg-stone-50 dark:hover:bg-gray-800 transition-colors min-h-[60px]
                   {activeConversationId === conv.id ? 'bg-stone-100 dark:bg-gray-800' : ''}"
            onclick={() => onSelect(conv.id)}
          >
            <div class="flex items-start justify-between gap-2">
              <div class="flex-1 min-w-0">
                <p class="text-sm font-medium text-stone-800 dark:text-gray-100 truncate">{conv.title}</p>
                <p class="text-xs text-stone-500 dark:text-gray-400 truncate mt-0.5">{conv.last_message_preview}</p>
              </div>
              <span class="text-xs text-stone-500 dark:text-gray-400 flex-shrink-0">
                {relativeTime(conv.last_message_at)}
              </span>
            </div>
          </button>

          {#if confirmDeleteId === conv.id}
            <div class="absolute inset-0 bg-white dark:bg-gray-900 flex items-center justify-between px-4
                        border-b border-stone-100 dark:border-gray-800">
              <span class="text-xs text-stone-600 dark:text-gray-300">{$t('chat.delete_confirmation')}</span>
              <div class="flex gap-2">
                <button
                  class="px-3 py-1 text-xs text-stone-500 dark:text-gray-400 min-h-[32px]"
                  onclick={() => confirmDeleteId = null}
                >
                  {$t('common.cancel')}
                </button>
                <button
                  class="px-3 py-1 text-xs text-[var(--color-danger)] font-medium min-h-[32px]"
                  onclick={() => { onDelete(conv.id); confirmDeleteId = null; }}
                >
                  {$t('common.delete')}
                </button>
              </div>
            </div>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</div>
