<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { Message, CitationView } from '$lib/types/chat';
  import { renderSafeMarkdown } from '$lib/utils/markdown';
  import CitationChip from './CitationChip.svelte';
  import ConfidenceIndicator from './ConfidenceIndicator.svelte';
  import FeedbackWidget from './FeedbackWidget.svelte';
  import Avatar from '$lib/components/ui/Avatar.svelte';

  interface Props {
    message: Message;
  }
  let { message }: Props = $props();

  let isPatient = $derived(message.role === 'patient');

  let citations = $derived.by((): CitationView[] => {
    if (!message.source_chunks) return [];
    try {
      return JSON.parse(message.source_chunks);
    } catch {
      return [];
    }
  });

  function formatTime(timestamp: string): string {
    const date = new Date(timestamp);
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }
</script>

{#if isPatient}
  <div class="flex justify-end">
    <div class="max-w-[80%] flex flex-col items-end">
      <div class="bg-[var(--color-interactive)] text-white rounded-2xl rounded-br-md px-4 py-3">
        <p class="text-base leading-relaxed whitespace-pre-wrap">{message.content}</p>
      </div>
      <span class="text-xs text-stone-500 mt-1 mr-1">{formatTime(message.timestamp)}</span>
    </div>
  </div>
{:else}
  <div class="flex items-start gap-2">
    <div class="flex-shrink-0 mt-1">
      <Avatar name={$t('chat.avatar_initial')} variant="ai" size="sm" />
    </div>
    <div class="max-w-[85%] flex flex-col items-start">
      <!-- Spec 48 [CA-03]: Render AI messages with safe markdown -->
      <div class="bg-white border border-stone-100 rounded-2xl rounded-bl-md px-4 py-3 shadow-sm
                  text-stone-800 text-base leading-relaxed">
        {@html renderSafeMarkdown(message.content)}
      </div>

      {#if citations.length > 0}
        <div class="flex flex-wrap gap-2 mt-2 ml-1">
          {#each citations as citation}
            <CitationChip {citation} />
          {/each}
        </div>
      {/if}

      {#if message.confidence !== null}
        <div class="mt-2 ml-1">
          <ConfidenceIndicator confidence={message.confidence} />
        </div>
      {/if}

      {#if message.confidence !== null}
        <div class="mt-2 ml-1">
          <FeedbackWidget
            messageId={message.id}
            currentFeedback={message.feedback}
          />
        </div>
      {/if}

      <span class="text-xs text-stone-500 mt-1 ml-1">{formatTime(message.timestamp)}</span>
    </div>
  </div>
{/if}
