<script lang="ts">
  import type { ConversationSummary } from '$lib/types/chat';

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
    if (diffMins < 1) return 'Just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    const diffHours = Math.floor(diffMins / 60);
    if (diffHours < 24) return `${diffHours}h ago`;
    const diffDays = Math.floor(diffHours / 24);
    if (diffDays < 7) return `${diffDays}d ago`;
    return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
  }
</script>

<div class="flex flex-col h-full">
  <div class="px-4 py-4 border-b border-stone-200">
    <h2 class="text-lg font-bold text-stone-800">Conversations</h2>
    <button
      class="mt-2 w-full px-4 py-3 bg-teal-600 text-white rounded-xl
             font-medium text-sm hover:bg-teal-700 min-h-[44px]"
      onclick={onNewConversation}
    >
      + New conversation
    </button>
  </div>

  <div class="flex-1 overflow-y-auto">
    {#if conversations.length === 0}
      <div class="px-4 py-8 text-center">
        <p class="text-sm text-stone-400">No conversations yet</p>
      </div>
    {:else}
      {#each conversations as conv (conv.id)}
        <div class="relative">
          <button
            class="w-full text-left px-4 py-3 border-b border-stone-100
                   hover:bg-stone-50 transition-colors min-h-[60px]
                   {activeConversationId === conv.id ? 'bg-stone-100' : ''}"
            onclick={() => onSelect(conv.id)}
          >
            <div class="flex items-start justify-between gap-2">
              <div class="flex-1 min-w-0">
                <p class="text-sm font-medium text-stone-800 truncate">{conv.title}</p>
                <p class="text-xs text-stone-500 truncate mt-0.5">{conv.last_message_preview}</p>
              </div>
              <span class="text-xs text-stone-400 flex-shrink-0">
                {relativeTime(conv.last_message_at)}
              </span>
            </div>
          </button>

          {#if confirmDeleteId === conv.id}
            <div class="absolute inset-0 bg-white flex items-center justify-between px-4
                        border-b border-stone-100">
              <span class="text-xs text-stone-600">Delete this conversation?</span>
              <div class="flex gap-2">
                <button
                  class="px-3 py-1 text-xs text-stone-500 min-h-[32px]"
                  onclick={() => confirmDeleteId = null}
                >
                  Cancel
                </button>
                <button
                  class="px-3 py-1 text-xs text-red-600 font-medium min-h-[32px]"
                  onclick={() => { onDelete(conv.id); confirmDeleteId = null; }}
                >
                  Delete
                </button>
              </div>
            </div>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</div>
