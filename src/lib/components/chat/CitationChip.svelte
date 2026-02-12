<script lang="ts">
  import type { CitationView } from '$lib/types/chat';

  interface Props {
    citation: CitationView;
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { citation, onNavigate }: Props = $props();

  let showPanel = $state(false);

  let displayLabel = $derived(
    citation.professional_name
      ?? citation.document_title
      ?? 'Source document'
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
  aria-label="View source: {displayLabel}"
>
  <span class="w-1.5 h-1.5 rounded-full bg-blue-400 flex-shrink-0"></span>
  <span class="truncate max-w-[140px]">{displayLabel}</span>
  {#if displayDate}
    <span class="text-stone-400">- {displayDate}</span>
  {/if}
</button>

{#if showPanel}
  <div class="fixed inset-0 z-50 flex flex-col justify-end">
    <button
      class="absolute inset-0 bg-black/30"
      onclick={() => showPanel = false}
      aria-label="Close citation panel"
    ></button>

    <div class="relative bg-white rounded-t-2xl shadow-xl max-h-[60vh] overflow-y-auto
                animate-slide-up">
      <div class="flex justify-center py-3">
        <div class="w-10 h-1 rounded-full bg-stone-300"></div>
      </div>

      <div class="px-6 pb-8">
        <div class="mb-4">
          <h3 class="text-lg font-medium text-stone-800">
            {citation.document_title}
          </h3>
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
          <h4 class="text-xs font-medium text-stone-400 uppercase mb-2">Source excerpt</h4>
          <p class="text-sm text-stone-700 leading-relaxed bg-stone-50 rounded-lg p-3 border border-stone-100">
            {citation.chunk_text}
          </p>
        </div>

        <div class="mb-6">
          <h4 class="text-xs font-medium text-stone-400 uppercase mb-2">Relevance</h4>
          <div class="flex items-center gap-2">
            <div class="flex-1 h-2 bg-stone-100 rounded-full overflow-hidden">
              <div
                class="h-full bg-teal-600 rounded-full transition-all"
                style="width: {relevanceWidth}%"
              ></div>
            </div>
            <span class="text-xs text-stone-500">{relevanceWidth}%</span>
          </div>
        </div>

        <button
          class="w-full px-6 py-3 bg-stone-100 text-stone-700 rounded-xl font-medium
                 hover:bg-stone-200 transition-colors min-h-[44px]"
          onclick={() => {
            showPanel = false;
            onNavigate('document-detail', { documentId: citation.document_id });
          }}
        >
          View full document
        </button>
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
