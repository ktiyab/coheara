<!-- L5-01: Critical Lab Alert Banner — persistent amber banner for Home/Chat -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { getCriticalAlerts, dismissCritical } from '$lib/api/trust';
  import type { CriticalLabAlert } from '$lib/types/trust';

  interface Props {
    onViewDocument?: (documentId: string) => void;
  }
  let { onViewDocument }: Props = $props();

  let alerts: CriticalLabAlert[] = $state([]);
  let dismissingId: string | null = $state(null);
  let dismissStep: 'idle' | 'confirm' | 'reason' = $state('idle');
  let dismissReason = $state('');
  let error: string | null = $state(null);

  let activeAlerts = $derived(alerts.filter((a) => !a.dismissed));

  onMount(async () => {
    try {
      alerts = await getCriticalAlerts();
    } catch (e) {
      console.error('Failed to load critical alerts:', e);
    }
  });

  function startDismiss(alertId: string) {
    dismissingId = alertId;
    dismissStep = 'confirm';
    dismissReason = '';
    error = null;
  }

  function cancelDismiss() {
    dismissingId = null;
    dismissStep = 'idle';
    dismissReason = '';
    error = null;
  }

  async function handleConfirmStep() {
    if (!dismissingId) return;
    try {
      await dismissCritical({ alert_id: dismissingId, step: 'AskConfirmation' });
      dismissStep = 'reason';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function handleDismissConfirm() {
    if (!dismissingId) return;
    if (!dismissReason.trim()) {
      error = 'Please provide a reason';
      return;
    }
    try {
      await dismissCritical({
        alert_id: dismissingId,
        step: { ConfirmDismissal: { reason: dismissReason.trim() } },
      });
      alerts = alerts.map((a) => (a.id === dismissingId ? { ...a, dismissed: true } : a));
      cancelDismiss();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function formatDate(dateStr: string): string {
    try {
      return new Date(dateStr).toLocaleDateString();
    } catch {
      return dateStr;
    }
  }
</script>

{#each activeAlerts as alert (alert.id)}
  <div class="mx-4 mb-2 rounded-xl bg-amber-50 border border-amber-200 p-4">
    <div class="flex items-start gap-3">
      <div class="flex-1">
        <p class="text-sm font-medium text-amber-800">
          Your lab report from {formatDate(alert.lab_date)} flags
          <strong>{alert.test_name}</strong> as needing prompt attention.
          Please contact your doctor or pharmacist soon.
        </p>
        <p class="text-xs text-amber-600 mt-1">
          {alert.test_name}: {alert.value} {alert.unit}
          (reference: {alert.reference_range})
        </p>
      </div>

      <div class="flex gap-2 shrink-0">
        {#if onViewDocument}
          <button
            class="px-3 py-1.5 text-xs text-amber-700 bg-amber-100 rounded-lg
                   min-h-[32px] hover:bg-amber-200"
            onclick={() => onViewDocument?.(alert.document_id)}
          >
            View
          </button>
        {/if}
        <button
          class="px-3 py-1.5 text-xs text-amber-700 bg-white border border-amber-200
                 rounded-lg min-h-[32px] hover:bg-amber-50"
          onclick={() => startDismiss(alert.id)}
        >
          Dismiss
        </button>
      </div>
    </div>

    <!-- 2-Step Dismissal Dialog -->
    {#if dismissingId === alert.id}
      <div class="mt-3 pt-3 border-t border-amber-200">
        {#if dismissStep === 'confirm'}
          <p class="text-sm text-amber-800 mb-3">Has your doctor addressed this?</p>
          <div class="flex gap-2">
            <button
              class="flex-1 px-3 py-2 text-sm bg-amber-100 text-amber-800 rounded-lg
                     min-h-[44px] hover:bg-amber-200"
              onclick={handleConfirmStep}
            >
              Yes, my doctor has seen this
            </button>
            <button
              class="px-3 py-2 text-sm bg-white border border-amber-200 text-amber-700
                     rounded-lg min-h-[44px] hover:bg-amber-50"
              onclick={cancelDismiss}
            >
              Not yet
            </button>
          </div>
        {:else if dismissStep === 'reason'}
          <p class="text-sm text-amber-800 mb-2">
            Please confirm: "My doctor has addressed this lab result"
          </p>
          <input
            type="text"
            class="w-full px-3 py-2 rounded-lg border border-amber-200 text-stone-700
                   text-sm mb-2 min-h-[44px]"
            bind:value={dismissReason}
            placeholder="e.g. Discussed with Dr. Smith on Jan 15"
          />
          {#if error}
            <p class="text-red-600 text-xs mb-2">{error}</p>
          {/if}
          <div class="flex gap-2">
            <button
              class="flex-1 px-3 py-2 text-sm bg-amber-600 text-white rounded-lg
                     min-h-[44px] disabled:opacity-50"
              disabled={!dismissReason.trim()}
              onclick={handleDismissConfirm}
            >
              Confirm
            </button>
            <button
              class="px-3 py-2 text-sm bg-white border border-amber-200 text-amber-700
                     rounded-lg min-h-[44px]"
              onclick={cancelDismiss}
            >
              Cancel
            </button>
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/each}
