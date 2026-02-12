<!-- L3-04: Confirm/reject action bar with flagged-fields warning and reject dialog. -->
<script lang="ts">
  import { confirmReview, rejectReview } from '$lib/api/review';
  import type { FieldCorrection, EntitiesStoredSummary } from '$lib/types/review';

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

  async function handleConfirm() {
    if (flaggedFields > 0 && !showFlaggedWarning) {
      showFlaggedWarning = true;
      return;
    }

    confirming = true;
    try {
      const result = await confirmReview(documentId, corrections);
      onConfirmSuccess({
        status: result.status,
        entities: result.entities_stored,
      });
    } catch (e) {
      console.error('Confirm failed:', e);
      alert(e instanceof Error ? e.message : 'Something went wrong while saving.');
    } finally {
      confirming = false;
      showFlaggedWarning = false;
    }
  }

  async function handleReject(action: 'retry' | 'remove') {
    rejecting = true;
    try {
      await rejectReview(documentId, rejectReason || null, action);
      showRejectDialog = false;
      onReject();
    } catch (e) {
      console.error('Reject failed:', e);
      alert(e instanceof Error ? e.message : 'Something went wrong.');
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
        Some fields need checking
      </h3>
      <p class="text-stone-600 text-sm mb-4">
        There {flaggedFields === 1 ? 'is' : 'are'} {flaggedFields} field{flaggedFields === 1 ? '' : 's'}
        I wasn't sure about. Would you like to check {flaggedFields === 1 ? 'it' : 'them'} first?
      </p>
      <div class="flex gap-3">
        <button
          class="flex-1 px-4 py-3 border border-stone-200 rounded-xl text-stone-700
                 hover:bg-stone-50 min-h-[44px]"
          onclick={() => showFlaggedWarning = false}
        >
          Check flagged fields
        </button>
        <button
          class="flex-1 px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
                 font-medium hover:brightness-110 min-h-[44px]"
          onclick={handleConfirm}
          disabled={confirming}
        >
          {confirming ? 'Saving...' : 'Confirm anyway'}
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
        What would you like to do?
      </h3>
      <p class="text-stone-600 text-sm mb-4">
        Would you like to try again or remove this document?
      </p>

      <div class="mb-4">
        <label for="reject-reason" class="text-sm text-stone-500 block mb-1">
          What went wrong? (optional)
        </label>
        <input
          id="reject-reason"
          type="text"
          bind:value={rejectReason}
          placeholder="e.g., Wrong document, too blurry..."
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
          {rejecting ? 'Processing...' : 'Try again'}
        </button>
        <button
          class="w-full px-4 py-3 border border-red-200 rounded-xl text-red-700
                 hover:bg-red-50 min-h-[44px]"
          onclick={() => handleReject('remove')}
          disabled={rejecting}
        >
          Remove document
        </button>
        <button
          class="w-full px-4 py-3 text-stone-500 min-h-[44px]"
          onclick={() => showRejectDialog = false}
        >
          Cancel
        </button>
      </div>
    </div>
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
    Not right, try again
  </button>
  <button
    class="flex-1 px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
           font-medium hover:brightness-110 min-h-[44px]"
    onclick={handleConfirm}
    disabled={confirming || rejecting}
  >
    {confirming ? 'Saving...' : corrections.length > 0 ? `Confirm (${corrections.length} corrected)` : 'Looks good to me'}
  </button>
</div>
