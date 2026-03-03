<!-- L3-04: Confirm/reject/delete action bar with flagged-entities warning. -->
<script lang="ts">
  import { tick } from 'svelte';
  import { t } from 'svelte-i18n';
  import { confirmReview, rejectReview } from '$lib/api/review';
  import { deleteDocument } from '$lib/api/import';
  import type { FieldCorrection, ExcludedEntity, EntitiesStoredSummary } from '$lib/types/review';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import { trapFocus, autoFocusFirst } from '$lib/utils/focus-trap';

  interface Props {
    documentId: string;
    corrections: FieldCorrection[];
    excludedEntities: ExcludedEntity[];
    flaggedEntities: number;
    onConfirmSuccess: (result: { status: string; entities: EntitiesStoredSummary }) => void;
    onReject: () => void;
  }
  let { documentId, corrections, excludedEntities, flaggedEntities, onConfirmSuccess, onReject }: Props = $props();

  let confirming = $state(false);
  let rejecting = $state(false);
  let deleting = $state(false);
  let showFlaggedWarning = $state(false);
  let errorMessage: string | null = $state(null);
  let errorGuidance: string | undefined = $state(undefined);

  let flaggedDialogEl: HTMLDivElement | undefined = $state(undefined);

  $effect(() => {
    if (flaggedDialogEl) {
      tick().then(() => { if (flaggedDialogEl) autoFocusFirst(flaggedDialogEl); });
    }
  });

  let busy = $derived(confirming || rejecting || deleting);

  async function handleConfirm() {
    if (flaggedEntities > 0 && !showFlaggedWarning) {
      showFlaggedWarning = true;
      return;
    }

    confirming = true;
    errorMessage = null;
    try {
      const result = await confirmReview(documentId, corrections, excludedEntities);
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

  async function handleReject() {
    rejecting = true;
    errorMessage = null;
    try {
      await rejectReview(documentId, null, 'retry');
      onReject();
    } catch (e) {
      console.error('Reject failed:', e);
      errorMessage = e instanceof Error ? e.message : String(e);
      errorGuidance = $t('review.reject_error_guidance');
    } finally {
      rejecting = false;
    }
  }

  async function handleDelete() {
    deleting = true;
    errorMessage = null;
    try {
      await deleteDocument(documentId);
      onReject();
    } catch (e) {
      console.error('Delete failed:', e);
      errorMessage = e instanceof Error ? e.message : String(e);
      errorGuidance = $t('review.delete_error_guidance');
    } finally {
      deleting = false;
    }
  }
</script>

<!-- Flagged entities warning overlay -->
{#if showFlaggedWarning}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="fixed inset-0 bg-black/30 flex items-end justify-center z-50 p-4"
       role="dialog" aria-modal="true" aria-label={$t('review.flagged_dialog_aria')}
       tabindex="-1"
       bind:this={flaggedDialogEl}
       onkeydown={(e) => { if (e.key === 'Escape') showFlaggedWarning = false; if (flaggedDialogEl) trapFocus(e, flaggedDialogEl); }}>
    <div class="bg-white dark:bg-gray-900 rounded-2xl p-6 max-w-md w-full shadow-xl">
      <h3 class="text-lg font-semibold text-stone-800 dark:text-gray-100 mb-2">
        {$t('review.flagged_heading_entities')}
      </h3>
      <p class="text-stone-600 dark:text-gray-300 text-sm mb-4">
        {$t('review.flagged_description_entities', { values: { count: flaggedEntities } })}
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

<!-- Error banner -->
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

<!-- Action bar: Delete | Retry | Confirm -->
<div class="flex items-center gap-3 px-4 py-4 bg-stone-50 dark:bg-gray-950 shrink-0">
  <Button variant="danger" loading={deleting} disabled={confirming || rejecting} onclick={handleDelete}>
    {deleting ? $t('common.processing') : $t('common.delete')}
  </Button>
  <div class="flex-1"></div>
  <Button variant="secondary" loading={rejecting} disabled={confirming || deleting} onclick={handleReject}>
    {rejecting ? $t('common.processing') : $t('review.not_right')}
  </Button>
  <Button variant="primary" loading={confirming} disabled={rejecting || deleting} onclick={handleConfirm}>
    {confirming ? $t('common.saving') : corrections.length > 0 ? $t('review.confirm_corrected', { values: { count: corrections.length } }) : $t('review.looks_good')}
  </Button>
</div>
