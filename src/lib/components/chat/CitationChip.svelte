<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { CitationView } from '$lib/types/chat';

  import { navigation } from '$lib/stores/navigation.svelte';
  import Button from '$lib/components/ui/Button.svelte';

  interface Props {
    citation: CitationView;
  }
  let { citation }: Props = $props();

  let showPanel = $state(false);

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

  let relevanceWidth = $derived(Math.round(citation.relevance_score * 100));
</script>

<button
  class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full
         bg-stone-100 hover:bg-stone-200 border border-stone-200
         text-xs text-stone-700 transition-colors
         min-h-[32px]"
  onclick={() => showPanel = true}
  aria-label={$t('chat.citation_view_source_aria', { values: { label: displayLabel } })}
>
  <span class="w-1.5 h-1.5 rounded-full bg-[var(--color-info)] flex-shrink-0"></span>
  <span class="truncate max-w-[140px]">{displayLabel}</span>
  {#if displayDate}
    <span class="text-stone-500">- {displayDate}</span>
  {/if}
</button>

{#if showPanel}
  <div class="fixed inset-0 z-50 flex flex-col justify-end">
    <button
      class="absolute inset-0 bg-black/30"
      onclick={() => showPanel = false}
      aria-label={$t('chat.citation_close_panel')}
    ></button>

    <div class="relative bg-white rounded-t-2xl shadow-xl max-h-[60vh] overflow-y-auto
                animate-slide-up">
      <div class="flex justify-center py-3">
        <div class="w-10 h-1 rounded-full bg-stone-300"></div>
      </div>

      <div class="px-6 pb-8">
        <div class="mb-4">
          <h2 class="text-lg font-medium text-stone-800">
            {citation.document_title}
          </h2>
          <div class="flex items-center gap-2 mt-1 text-sm text-stone-500">
            {#if citation.professional_name}
              <span>{citation.professional_name}</span>
            {/if}
            {#if citation.document_date}
              <span>- {citation.document_date}</span>
            {/if}
          </div>
        </div>

        <div class="mb-4">
          <h3 class="text-xs font-medium text-stone-500 uppercase mb-2">{$t('chat.citation_excerpt_heading')}</h3>
          <p class="text-sm text-stone-700 leading-relaxed bg-stone-50 rounded-lg p-3 border border-stone-100">
            {citation.chunk_text}
          </p>
        </div>

        <div class="mb-6">
          <h3 class="text-xs font-medium text-stone-500 uppercase mb-2">{$t('chat.citation_relevance_heading')}</h3>
          <div class="flex items-center gap-2">
            <div class="flex-1 h-2 bg-stone-100 rounded-full overflow-hidden">
              <div
                class="h-full bg-[var(--color-interactive)] rounded-full transition-all"
                style="width: {relevanceWidth}%"
              ></div>
            </div>
            <span class="text-xs text-stone-500">{relevanceWidth}%</span>
          </div>
        </div>

        <Button variant="secondary" fullWidth onclick={() => {
            showPanel = false;
            navigation.navigate('document-detail', { documentId: citation.document_id });
          }}>
          {$t('chat.citation_view_full')}
        </Button>
      </div>
    </div>
  </div>
{/if}

<style>
  @keyframes slide-up {
    from { transform: translateY(100%); }
    to { transform: translateY(0); }
  }
  .animate-slide-up {
    animation: slide-up 0.3s ease-out;
  }
</style>
