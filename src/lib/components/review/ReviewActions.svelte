<!-- L3-04: Confirm/reject action bar with flagged-fields warning and reject dialog. -->
<script lang="ts">
  import { tick } from 'svelte';
  import { t } from 'svelte-i18n';
  import { confirmReview, rejectReview } from '$lib/api/review';
  import type { FieldCorrection, EntitiesStoredSummary } from '$lib/types/review';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import { trapFocus, autoFocusFirst } from '$lib/utils/focus-trap';

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

  let flaggedDialogEl: HTMLDivElement | undefined = $state(undefined);
  let rejectDialogEl: HTMLDivElement | undefined = $state(undefined);

  $effect(() => {
    if (flaggedDialogEl) {
      tick().then(() => { if (flaggedDialogEl) autoFocusFirst(flaggedDialogEl); });
    }
  });

  $effect(() => {
    if (rejectDialogEl) {
      tick().then(() => { if (rejectDialogEl) autoFocusFirst(rejectDialogEl); });
    }
  });

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
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="fixed inset-0 bg-black/30 flex items-end justify-center z-50 p-4"
       role="dialog" aria-modal="true" aria-label={$t('review.flagged_dialog_aria')}
       tabindex="-1"
       bind:this={flaggedDialogEl}
       onkeydown={(e) => { if (e.key === 'Escape') showFlaggedWarning = false; if (flaggedDialogEl) trapFocus(e, flaggedDialogEl); }}>
    <div class="bg-white dark:bg-gray-900 rounded-2xl p-6 max-w-md w-full shadow-xl">
      <h3 class="text-lg font-semibold text-stone-800 dark:text-gray-100 mb-2">
        {$t('review.flagged_heading')}
      </h3>
      <p class="text-stone-600 dark:text-gray-300 text-sm mb-4">
        {$t('review.flagged_description', { values: { count: flaggedFields } })}
      </p>
      <div class="flex gap-3">
        <Button variant="secondary" onclick={() => showFlaggedWarning = false}>
          {$t('review.check_flagged')}
        </Button>
        <Button variant="primary" loading={confirming} onclick={handleConfirm}>
          {confirming ? $t('common.saving') : $t('review.confirm_anyway')}
        </Button>
      </div>
    </div>
  </div>
{/if}

<!-- Reject dialog overlay -->
{#if showRejectDialog}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="fixed inset-0 bg-black/30 flex items-end justify-center z-50 p-4"
       role="dialog" aria-modal="true" aria-label={$t('review.reject_dialog_aria')}
       tabindex="-1"
       bind:this={rejectDialogEl}
       onkeydown={(e) => { if (e.key === 'Escape') showRejectDialog = false; if (rejectDialogEl) trapFocus(e, rejectDialogEl); }}>
    <div class="bg-white dark:bg-gray-900 rounded-2xl p-6 max-w-md w-full shadow-xl">
      <h3 class="text-lg font-semibold text-stone-800 dark:text-gray-100 mb-2">
        {$t('review.reject_heading')}
      </h3>
      <p class="text-stone-600 dark:text-gray-300 text-sm mb-4">
        {$t('review.reject_description')}
      </p>

      <div class="mb-4">
        <label for="reject-reason" class="text-sm text-stone-500 dark:text-gray-400 block mb-1">
          {$t('review.reject_reason_label')}
        </label>
        <input
          id="reject-reason"
          type="text"
          bind:value={rejectReason}
          placeholder={$t('review.reject_reason_placeholder')}
          class="w-full px-3 py-2 border border-stone-200 dark:border-gray-700 rounded-lg text-sm
                 bg-white dark:bg-gray-900 text-stone-700 dark:text-gray-200
                 focus:border-[var(--color-primary)] focus:outline-none min-h-[44px]"
        />
      </div>

      <div class="flex flex-col gap-2">
        <Button variant="secondary" fullWidth loading={rejecting} onclick={() => handleReject('retry')}>
          {rejecting ? $t('common.processing') : $t('review.try_again')}
        </Button>
        <Button variant="danger" fullWidth disabled={rejecting} onclick={() => handleReject('remove')}>
          {$t('review.remove_document')}
        </Button>
        <Button variant="ghost" fullWidth onclick={() => showRejectDialog = false}>
          {$t('common.cancel')}
        </Button>
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
<div class="flex gap-3 px-4 py-4 bg-stone-50 dark:bg-gray-950 shrink-0">
  <Button variant="secondary" disabled={confirming || rejecting} onclick={() => showRejectDialog = true}>
    {$t('review.not_right')}
  </Button>
  <Button variant="primary" loading={confirming} disabled={rejecting} onclick={handleConfirm}>
    {confirming ? $t('common.saving') : corrections.length > 0 ? $t('review.confirm_corrected', { values: { count: corrections.length } }) : $t('review.looks_good')}
  </Button>
</div>
