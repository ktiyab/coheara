<!-- L3-04: Success screen after confirming review. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { EntitiesStoredSummary } from '$lib/types/review';
  import Button from '$lib/components/ui/Button.svelte';
  import { CheckCircleSolid } from 'flowbite-svelte-icons';

  interface Props {
    documentType: string;
    documentDate?: string | null;
    status: string;
    entities: EntitiesStoredSummary;
    correctionsApplied: number;
    onViewDocument: () => void;
    onBackToHome: () => void;
    onAskAi?: () => void;
  }
  let { documentType, documentDate, status, entities, correctionsApplied, onViewDocument, onBackToHome, onAskAi }: Props = $props();

  let entitySummary = $derived.by(() => {
    const parts: string[] = [];
    if (entities.medications > 0) parts.push($t('review.success_medications', { values: { count: entities.medications } }));
    if (entities.lab_results > 0) parts.push($t('review.success_lab_results', { values: { count: entities.lab_results } }));
    if (entities.diagnoses > 0) parts.push($t('review.success_diagnoses', { values: { count: entities.diagnoses } }));
    if (entities.allergies > 0) parts.push($t('review.success_allergies', { values: { count: entities.allergies } }));
    if (entities.procedures > 0) parts.push($t('review.success_procedures', { values: { count: entities.procedures } }));
    if (entities.referrals > 0) parts.push($t('review.success_referrals', { values: { count: entities.referrals } }));
    return parts.length > 0 ? parts.join(', ') : $t('review.success_no_entities');
  });
</script>

<div class="flex flex-col items-center justify-center h-screen bg-stone-50 dark:bg-gray-950 px-6">
  <div class="max-w-md w-full text-center">
    <div class="mb-6" aria-hidden="true">
      <CheckCircleSolid class="w-16 h-16 text-[var(--color-success)]" />
    </div>

    <h1 class="text-2xl font-semibold text-stone-800 dark:text-gray-100 mb-2">
      {$t('review.success_heading')}
    </h1>

    <p class="text-stone-600 dark:text-gray-300 mb-2">
      {documentType}
      {#if status === 'Corrected'}
        &middot; {$t('review.success_corrections', { values: { count: correctionsApplied } })}
      {/if}
    </p>

    <p class="text-sm text-stone-500 dark:text-gray-400 mb-8">
      {entitySummary} {$t('review.success_added')}
    </p>

    <div class="flex flex-col gap-3">
      {#if onAskAi}
        <!-- Spec 48 [CA-05]: Post-review AI CTA -->
        <Button variant="primary" fullWidth onclick={onAskAi}>
          {$t('review.success_ask_ai')}
        </Button>
      {/if}
      <Button variant={onAskAi ? 'secondary' : 'primary'} fullWidth onclick={onBackToHome}>
        {$t('review.success_back_home')}
      </Button>
      <Button variant="ghost" fullWidth onclick={onViewDocument}>
        {$t('review.success_view_document')}
      </Button>
    </div>
  </div>
</div>
