<script lang="ts">
  import type { Message, CitationView } from '$lib/types/chat';
  import CitationChip from './CitationChip.svelte';
  import ConfidenceIndicator from './ConfidenceIndicator.svelte';
  import FeedbackWidget from './FeedbackWidget.svelte';

  interface Props {
    message: Message;
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { message, onNavigate }: Props = $props();

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
      <div class="bg-teal-600 text-white rounded-2xl rounded-br-md px-4 py-3">
        <p class="text-base leading-relaxed whitespace-pre-wrap">{message.content}</p>
      </div>
      <span class="text-xs text-stone-400 mt-1 mr-1">{formatTime(message.timestamp)}</span>
    </div>
  </div>
{:else}
  <div class="flex items-start gap-2">
    <div class="w-8 h-8 rounded-full bg-teal-600 flex items-center justify-center
                text-white text-sm font-bold flex-shrink-0 mt-1">
      C
    </div>
    <div class="max-w-[85%] flex flex-col items-start">
      <div class="bg-white border border-stone-100 rounded-2xl rounded-bl-md px-4 py-3 shadow-sm">
        <p class="text-stone-800 text-base leading-relaxed whitespace-pre-wrap">{message.content}</p>
      </div>

      {#if citations.length > 0}
        <div class="flex flex-wrap gap-2 mt-2 ml-1">
          {#each citations as citation}
            <CitationChip {citation} {onNavigate} />
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

      <span class="text-xs text-stone-400 mt-1 ml-1">{formatTime(message.timestamp)}</span>
    </div>
  </div>
{/if}
