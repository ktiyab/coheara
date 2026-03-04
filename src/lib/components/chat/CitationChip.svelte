<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { CitationView } from '$lib/types/chat';
  import DocumentPreviewPanel from './DocumentPreviewPanel.svelte';

  interface Props {
    citation: CitationView;
  }
  let { citation }: Props = $props();

  let showPreview = $state(false);

  let displayLabel = $derived(
    citation.professional_name
      ?? citation.document_title
      ?? $t('chat.citation_source_document')
  );

  let displayDate = $derived.by(() => {
    if (!citation.document_date) return '';
    const date = new Date(citation.document_date);
    return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
  });
</script>

<!-- Citation chip button (compact, inline with message) -->
<button
  class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full
         bg-stone-100 dark:bg-gray-800 hover:bg-stone-200 dark:hover:bg-gray-700 border border-stone-200 dark:border-gray-700
         text-xs text-stone-700 dark:text-gray-200 transition-colors
         min-h-[32px]"
  onclick={() => showPreview = true}
  aria-label={$t('chat.citation_view_source_aria', { values: { label: displayLabel } })}
>
  <span class="w-1.5 h-1.5 rounded-full bg-[var(--color-info)] flex-shrink-0"></span>
  <span class="truncate max-w-[140px]">{displayLabel}</span>
  {#if displayDate}
    <span class="text-stone-500 dark:text-gray-400">- {displayDate}</span>
  {/if}
</button>

<!-- Document preview panel (slide-over, conversation stays open) -->
{#if showPreview}
  <DocumentPreviewPanel
    documentId={citation.document_id}
    highlightExcerpt={citation.chunk_text}
    onclose={() => showPreview = false}
  />
{/if}
