<!-- L3-04: Success screen after confirming review. -->
<script lang="ts">
  import type { EntitiesStoredSummary } from '$lib/types/review';

  interface Props {
    documentType: string;
    status: string;
    entities: EntitiesStoredSummary;
    correctionsApplied: number;
    onViewDocument: () => void;
    onBackToHome: () => void;
  }
  let { documentType, status, entities, correctionsApplied, onViewDocument, onBackToHome }: Props = $props();

  let entitySummary = $derived.by(() => {
    const parts: string[] = [];
    if (entities.medications > 0) parts.push(`${entities.medications} medication${entities.medications > 1 ? 's' : ''}`);
    if (entities.lab_results > 0) parts.push(`${entities.lab_results} lab result${entities.lab_results > 1 ? 's' : ''}`);
    if (entities.diagnoses > 0) parts.push(`${entities.diagnoses} ${entities.diagnoses > 1 ? 'diagnoses' : 'diagnosis'}`);
    if (entities.allergies > 0) parts.push(`${entities.allergies} ${entities.allergies > 1 ? 'allergies' : 'allergy'}`);
    if (entities.procedures > 0) parts.push(`${entities.procedures} procedure${entities.procedures > 1 ? 's' : ''}`);
    if (entities.referrals > 0) parts.push(`${entities.referrals} referral${entities.referrals > 1 ? 's' : ''}`);
    return parts.length > 0 ? parts.join(', ') : 'No entities';
  });
</script>

<div class="flex flex-col items-center justify-center h-screen bg-stone-50 px-6">
  <div class="max-w-md w-full text-center">
    <div class="text-5xl mb-6" aria-hidden="true">&#x2713;</div>

    <h1 class="text-2xl font-semibold text-stone-800 mb-2">
      Your document has been saved
    </h1>

    <p class="text-stone-600 mb-2">
      {documentType}
      {#if status === 'Corrected'}
        &middot; {correctionsApplied} correction{correctionsApplied === 1 ? '' : 's'} applied
      {/if}
    </p>

    <p class="text-sm text-stone-500 mb-8">
      {entitySummary} added to your profile.
    </p>

    <div class="flex flex-col gap-3">
      <button
        class="w-full px-6 py-3 bg-[var(--color-primary)] text-white rounded-xl
               font-medium hover:brightness-110 min-h-[44px]"
        onclick={onBackToHome}
      >
        Back to home
      </button>
      <button
        class="w-full px-6 py-3 border border-stone-200 rounded-xl text-stone-600
               hover:bg-stone-50 min-h-[44px]"
        onclick={onViewDocument}
      >
        View document
      </button>
    </div>
  </div>
</div>
