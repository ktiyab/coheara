<!-- CHAT-QUEUE-01: Inline queue status indicator below patient messages.
     Shows waiting/acquiring state when a message is queued for SLM processing. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { ChatQueueState } from '$lib/types/chat';

  interface Props {
    queueState: ChatQueueState;
    queuePosition?: number;
  }
  let { queueState, queuePosition }: Props = $props();
</script>

{#if queueState === 'Queued'}
  <div class="flex items-center gap-1.5 ml-10 mt-1 text-xs text-stone-400 dark:text-gray-400">
    <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <circle cx="12" cy="12" r="10" />
      <polyline points="12 6 12 12 16 14" />
    </svg>
    <span>{$t('chat.queue_waiting')}</span>
    {#if queuePosition && queuePosition > 1}
      <span class="text-stone-300 dark:text-gray-600">({$t('chat.queue_position', { values: { position: queuePosition } })})</span>
    {/if}
  </div>
{:else if queueState === 'Acquiring'}
  <div class="flex items-center gap-1.5 ml-10 mt-1 text-xs text-stone-400 dark:text-gray-400">
    <svg class="w-3.5 h-3.5 animate-spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path d="M21 12a9 9 0 1 1-6.219-8.56" />
    </svg>
    <span>{$t('chat.queue_acquiring')}</span>
  </div>
{/if}
