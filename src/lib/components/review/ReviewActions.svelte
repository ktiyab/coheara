<!-- L3-04: Confirm/reject action bar with flagged-fields warning and reject dialog. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { confirmReview, rejectReview } from '$lib/api/review';
  import type { FieldCorrection, EntitiesStoredSummary } from '$lib/types/review';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';

  interface Props {
    documentId: string;
    corrections: FieldCorrection[];
    flaggedFields: number;
    onConfirmSuccess: (result: { status: string; entities: EntitiesStoredSummary }) => void;
    onReject: () => void;
  }
  let { documentId, corrections, flaggedFields, onConfirmSuccess, onReject }: Props = $props();

  let confirming = $state(false);
  let rejecting = $state(false);
  let showFlaggedWarning = $state(false);
  let showRejectDialog = $state(false);
  let rejectReason = $state('');
  // R.2: Replace alert() with inline error banner
  let errorMessage: string | null = $state(null);
  let errorGuidance: string | undefined = $state(undefined);

  async function handleConfirm() {
    if (flaggedFields > 0 && !showFlaggedWarning) {
      showFlaggedWarning = true;
      return;
    }

    confirming = true;
    errorMessage = null;
    try {
      const result = await confirmReview(documentId, corrections);
      onConfirmSuccess({
        status: result.status,
        entities: result.entities_stored,
      });
    } catch (e) {
      console.error('Confirm failed:', e);
      errorMessage = e instanceof Error ? e.message : String(e);
      errorGuidance = $t('review.confirm_error_guidance');
    } finally {
      confirming = false;
      showFlaggedWarning = false;
    }
  }

  async function handleReject(action: 'retry' | 'remove') {
    rejecting = true;
    errorMessage = null;
    try {
      await rejectReview(documentId, rejectReason || null, action);
      showRejectDialog = false;
      onReject();
    } catch (e) {
      console.error('Reject failed:', e);
      errorMessage = e instanceof Error ? e.message : String(e);
      errorGuidance = $t('review.reject_error_guidance');
    } finally {
      rejecting = false;
    }
  }
</script>

<!-- Flagged fields warning overlay -->
{#if showFlaggedWarning}
  <div class="fixed inset-0 bg-black/30 flex items-end justify-center z-50 p-4"
       role="dialog" aria-modal="true" aria-label="Flagged fields reminder">
    <div class="bg-white rounded-2xl p-6 max-w-md w-full shadow-xl">
      <h3 class="text-lg font-semibold text-stone-800 mb-2">
        {$t('review.flagged_heading')}
      </h3>
      <p class="text-stone-600 text-sm mb-4">
        {$t('review.flagged_description', { values: { count: flaggedFields } })}
      </p>
      <div class="flex gap-3">
        <button
          class="flex-1 px-4 py-3 border border-stone-200 rounded-xl text-stone-700
                 hover:bg-stone-50 min-h-[44px]"
          onclick={() => showFlaggedWarning = false}
        >
          {$t('review.check_flagged')}
        </button>
        <button
          class="flex-1 px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
                 font-medium hover:brightness-110 min-h-[44px]"
          onclick={handleConfirm}
          disabled={confirming}
        >
          {confirming ? $t('common.saving') : $t('review.confirm_anyway')}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Reject dialog overlay -->
{#if showRejectDialog}
  <div class="fixed inset-0 bg-black/30 flex items-end justify-center z-50 p-4"
       role="dialog" aria-modal="true" aria-label="Reject review">
    <div class="bg-white rounded-2xl p-6 max-w-md w-full shadow-xl">
      <h3 class="text-lg font-semibold text-stone-800 mb-2">
        {$t('review.reject_heading')}
      </h3>
      <p class="text-stone-600 text-sm mb-4">
        {$t('review.reject_description')}
      </p>

      <div class="mb-4">
        <label for="reject-reason" class="text-sm text-stone-500 block mb-1">
          {$t('review.reject_reason_label')}
        </label>
        <input
          id="reject-reason"
          type="text"
          bind:value={rejectReason}
          placeholder={$t('review.reject_reason_placeholder')}
          class="w-full px-3 py-2 border border-stone-200 rounded-lg text-sm
                 focus:border-[var(--color-primary)] focus:outline-none min-h-[44px]"
        />
      </div>

      <div class="flex flex-col gap-2">
        <button
          class="w-full px-4 py-3 border border-stone-200 rounded-xl text-stone-700
                 hover:bg-stone-50 min-h-[44px]"
          onclick={() => handleReject('retry')}
          disabled={rejecting}
        >
          {rejecting ? $t('common.processing') : $t('review.try_again')}
        </button>
        <button
          class="w-full px-4 py-3 border border-red-200 rounded-xl text-red-700
                 hover:bg-red-50 min-h-[44px]"
          onclick={() => handleReject('remove')}
          disabled={rejecting}
        >
          {$t('review.remove_document')}
        </button>
        <button
          class="w-full px-4 py-3 text-stone-500 min-h-[44px]"
          onclick={() => showRejectDialog = false}
        >
          {$t('common.cancel')}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- R.2: Error banner (replaces alert()) -->
{#if errorMessage}
  <div class="px-4 pt-3">
    <ErrorBanner
      message={errorMessage}
      severity="error"
      guidance={errorGuidance}
      onDismiss={() => { errorMessage = null; }}
    />
  </div>
{/if}

<!-- Action bar -->
<div class="flex gap-3 px-4 py-4 bg-white border-t border-stone-200 shrink-0">
  <button
    class="flex-1 px-4 py-3 border border-stone-200 rounded-xl text-stone-600
           hover:bg-stone-50 min-h-[44px]"
    onclick={() => showRejectDialog = true}
    disabled={confirming || rejecting}
  >
    {$t('review.not_right')}
  </button>
  <button
    class="flex-1 px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
           font-medium hover:brightness-110 min-h-[44px]"
    onclick={handleConfirm}
    disabled={confirming || rejecting}
  >
    {confirming ? $t('common.saving') : corrections.length > 0 ? $t('review.confirm_corrected', { values: { count: corrections.length } }) : $t('review.looks_good')}
  </button>
</div>
