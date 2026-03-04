<script lang="ts">
  import { t } from 'svelte-i18n';
  import { getDocumentDetail } from '$lib/api/documents';
  import type { DocumentDetail } from '$lib/types/documents';
  import Button from '$lib/components/ui/Button.svelte';

  interface Props {
    documentId: string;
    highlightExcerpt?: string;
    onclose: () => void;
  }
  let { documentId, highlightExcerpt, onclose }: Props = $props();

  let doc = $state<DocumentDetail | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  $effect(() => {
    loading = true;
    error = null;
    getDocumentDetail(documentId)
      .then((d) => { doc = d; })
      .catch((e) => { error = String(e); })
      .finally(() => { loading = false; });
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose();
  }

  function formatDate(dateStr: string | null): string {
    if (!dateStr) return '';
    const d = new Date(dateStr);
    return d.toLocaleDateString([], { year: 'numeric', month: 'short', day: 'numeric' });
  }

  let entityCount = $derived.by(() => {
    if (!doc) return 0;
    return doc.medications.length + doc.lab_results.length + doc.diagnoses.length
      + doc.allergies.length + doc.procedures.length + doc.referrals.length;
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Backdrop -->
<div class="fixed inset-0 z-50 flex justify-end" role="dialog" aria-modal="true" aria-label={$t('chat.document_preview_title')}>
  <button
    class="absolute inset-0 bg-black/30"
    onclick={onclose}
    aria-label={$t('chat.citation_close_panel')}
  ></button>

  <!-- Slide-over panel (right side, Apple-style) -->
  <div class="relative w-full max-w-md bg-white dark:bg-gray-900 shadow-2xl overflow-y-auto animate-slide-in">
    <!-- Header -->
    <div class="sticky top-0 z-10 bg-white/95 dark:bg-gray-900/95 backdrop-blur border-b border-stone-100 dark:border-gray-800 px-5 py-4">
      <div class="flex items-center justify-between">
        <h2 class="text-lg font-semibold text-stone-800 dark:text-gray-100 truncate pr-4">
          {#if loading}
            {$t('chat.document_preview_loading')}
          {:else if doc}
            {doc.title}
          {:else}
            {$t('chat.document_preview_error')}
          {/if}
        </h2>
        <button
          class="p-2 -mr-2 rounded-full hover:bg-stone-100 dark:hover:bg-gray-800 transition-colors"
          onclick={onclose}
          aria-label={$t('chat.citation_close_panel')}
        >
          <svg class="w-5 h-5 text-stone-500 dark:text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      {#if doc}
        <div class="flex items-center gap-2 mt-1 text-sm text-stone-500 dark:text-gray-400">
          {#if doc.professional_name}
            <span>{doc.professional_name}</span>
          {/if}
          {#if doc.document_date}
            <span>{doc.professional_name ? '-' : ''} {formatDate(doc.document_date)}</span>
          {/if}
          <span class="px-2 py-0.5 rounded-full text-xs bg-stone-100 dark:bg-gray-800 capitalize">{doc.document_type}</span>
        </div>
      {/if}
    </div>

    <!-- Content -->
    <div class="px-5 py-4 space-y-5">
      {#if loading}
        <div class="flex items-center justify-center py-12">
          <div class="w-6 h-6 border-2 border-stone-300 border-t-[var(--color-interactive)] rounded-full animate-spin"></div>
        </div>
      {:else if error}
        <p class="text-sm text-red-600 dark:text-red-400">{error}</p>
      {:else if doc}
        <!-- Cited excerpt highlight -->
        {#if highlightExcerpt}
          <section>
            <h3 class="text-xs font-medium text-stone-500 dark:text-gray-400 uppercase mb-2">{$t('chat.citation_excerpt_heading')}</h3>
            <div class="bg-amber-50 dark:bg-amber-950/30 border border-amber-200 dark:border-amber-900/50 rounded-lg p-3">
              <p class="text-sm text-stone-700 dark:text-gray-200 leading-relaxed italic">"{highlightExcerpt}"</p>
            </div>
          </section>
        {/if}

        <!-- Medications -->
        {#if doc.medications.length > 0}
          <section>
            <h3 class="text-xs font-medium text-stone-500 dark:text-gray-400 uppercase mb-2">
              {$t('chat.preview_medications')} ({doc.medications.length})
            </h3>
            <div class="space-y-2">
              {#each doc.medications as med}
                <div class="bg-stone-50 dark:bg-gray-950 rounded-lg p-3 border border-stone-100 dark:border-gray-800">
                  <div class="font-medium text-sm text-stone-800 dark:text-gray-100">
                    {med.generic_name}{med.brand_name ? ` (${med.brand_name})` : ''}
                  </div>
                  <div class="text-xs text-stone-500 dark:text-gray-400 mt-0.5">
                    {med.dose} - {med.frequency} - {med.route}
                  </div>
                  <span class="inline-block mt-1 px-1.5 py-0.5 rounded text-xs {med.status === 'Active' ? 'bg-green-100 text-green-700 dark:bg-green-950 dark:text-green-300' : 'bg-stone-100 text-stone-500 dark:bg-gray-800 dark:text-gray-400'}">
                    {med.status}
                  </span>
                </div>
              {/each}
            </div>
          </section>
        {/if}

        <!-- Lab Results -->
        {#if doc.lab_results.length > 0}
          <section>
            <h3 class="text-xs font-medium text-stone-500 dark:text-gray-400 uppercase mb-2">
              {$t('chat.preview_lab_results')} ({doc.lab_results.length})
            </h3>
            <div class="space-y-2">
              {#each doc.lab_results as lab}
                <div class="bg-stone-50 dark:bg-gray-950 rounded-lg p-3 border border-stone-100 dark:border-gray-800">
                  <div class="flex items-center justify-between">
                    <span class="font-medium text-sm text-stone-800 dark:text-gray-100">{lab.test_name}</span>
                    {#if lab.abnormal_flag !== 'Normal'}
                      <span class="px-1.5 py-0.5 rounded text-xs {lab.abnormal_flag.includes('Critical') ? 'bg-red-100 text-red-700 dark:bg-red-950 dark:text-red-300' : 'bg-amber-100 text-amber-700 dark:bg-amber-950 dark:text-amber-300'}">
                        {lab.abnormal_flag}
                      </span>
                    {/if}
                  </div>
                  <div class="text-sm text-stone-700 dark:text-gray-200 mt-0.5">
                    {lab.value ?? lab.value_text ?? '-'}{lab.unit ? ` ${lab.unit}` : ''}
                    {#if lab.reference_range_low != null && lab.reference_range_high != null}
                      <span class="text-xs text-stone-400 dark:text-gray-400 ml-1">
                        (ref: {lab.reference_range_low}-{lab.reference_range_high})
                      </span>
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          </section>
        {/if}

        <!-- Diagnoses -->
        {#if doc.diagnoses.length > 0}
          <section>
            <h3 class="text-xs font-medium text-stone-500 dark:text-gray-400 uppercase mb-2">
              {$t('chat.preview_diagnoses')} ({doc.diagnoses.length})
            </h3>
            <div class="space-y-2">
              {#each doc.diagnoses as dx}
                <div class="bg-stone-50 dark:bg-gray-950 rounded-lg p-3 border border-stone-100 dark:border-gray-800">
                  <div class="font-medium text-sm text-stone-800 dark:text-gray-100">{dx.name}</div>
                  <div class="flex items-center gap-2 mt-0.5">
                    {#if dx.icd_code}
                      <span class="text-xs text-stone-400 dark:text-gray-400">ICD: {dx.icd_code}</span>
                    {/if}
                    <span class="inline-block px-1.5 py-0.5 rounded text-xs {dx.status === 'Active' ? 'bg-green-100 text-green-700 dark:bg-green-950 dark:text-green-300' : 'bg-stone-100 text-stone-500 dark:bg-gray-800 dark:text-gray-400'}">
                      {dx.status}
                    </span>
                  </div>
                </div>
              {/each}
            </div>
          </section>
        {/if}

        <!-- Allergies -->
        {#if doc.allergies.length > 0}
          <section>
            <h3 class="text-xs font-medium text-stone-500 dark:text-gray-400 uppercase mb-2">
              {$t('chat.preview_allergies')} ({doc.allergies.length})
            </h3>
            <div class="space-y-2">
              {#each doc.allergies as allergy}
                <div class="bg-stone-50 dark:bg-gray-950 rounded-lg p-3 border border-stone-100 dark:border-gray-800">
                  <div class="font-medium text-sm text-stone-800 dark:text-gray-100">{allergy.allergen}</div>
                  <div class="flex items-center gap-2 mt-0.5">
                    {#if allergy.reaction}
                      <span class="text-xs text-stone-500 dark:text-gray-400">{allergy.reaction}</span>
                    {/if}
                    <span class="px-1.5 py-0.5 rounded text-xs {allergy.severity === 'LifeThreatening' || allergy.severity === 'Severe' ? 'bg-red-100 text-red-700 dark:bg-red-950 dark:text-red-300' : 'bg-amber-100 text-amber-700 dark:bg-amber-950 dark:text-amber-300'}">
                      {allergy.severity}
                    </span>
                  </div>
                </div>
              {/each}
            </div>
          </section>
        {/if}

        <!-- Empty state -->
        {#if entityCount === 0}
          <div class="text-center py-8 text-sm text-stone-400 dark:text-gray-400">
            {$t('chat.preview_no_entities')}
          </div>
        {/if}

        <!-- Footer: link to full document detail -->
        <div class="pt-4 border-t border-stone-100 dark:border-gray-800">
          <p class="text-xs text-stone-400 dark:text-gray-400 text-center">
            {$t('chat.preview_entity_count', { values: { count: entityCount } })}
          </p>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  @keyframes slide-in {
    from { transform: translateX(100%); }
    to { transform: translateX(0); }
  }
  .animate-slide-in {
    animation: slide-in 0.25s ease-out;
  }
</style>
