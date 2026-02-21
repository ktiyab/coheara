<!-- L5-01: Critical Lab Alert Banner — persistent amber banner for Home/Chat -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t, locale } from 'svelte-i18n';
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
      error = $t('critical.reason_required');
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
      return new Date(dateStr).toLocaleDateString($locale ?? undefined);
    } catch {
      return dateStr;
    }
  }
</script>

{#each activeAlerts as alert (alert.id)}
  <div class="mx-4 mb-2 rounded-xl bg-[var(--color-warning-50)] border border-[var(--color-warning-200)] p-4">
    <div class="flex items-start gap-3">
      <div class="flex-1">
        <p class="text-sm font-medium text-[var(--color-warning-800)]">
          {$t('critical.lab_alert', { values: { date: formatDate(alert.lab_date), testName: alert.test_name } })}
          {$t('critical.contact_doctor')}
        </p>
        <p class="text-xs text-[var(--color-warning)] mt-1">
          {alert.test_name}: {alert.value} {alert.unit}
          {$t('critical.reference_range', { values: { range: alert.reference_range } })}
        </p>
      </div>

      <div class="flex gap-2 shrink-0">
        {#if onViewDocument}
          <button
            class="px-3 py-1.5 text-xs text-[var(--color-warning-800)] bg-[var(--color-warning-200)] rounded-lg
                   min-h-[32px] hover:bg-[var(--color-warning-200)]"
            onclick={() => onViewDocument?.(alert.document_id)}
          >
            {$t('critical.view_button')}
          </button>
        {/if}
        <button
          class="px-3 py-1.5 text-xs text-[var(--color-warning-800)] bg-white dark:bg-gray-900 border border-[var(--color-warning-200)]
                 rounded-lg min-h-[32px] hover:bg-[var(--color-warning-50)]"
          onclick={() => startDismiss(alert.id)}
        >
          {$t('critical.dismiss_button')}
        </button>
      </div>
    </div>

    <!-- 2-Step Dismissal Dialog -->
    {#if dismissingId === alert.id}
      <div class="mt-3 pt-3 border-t border-[var(--color-warning-200)]">
        {#if dismissStep === 'confirm'}
          <p class="text-sm text-[var(--color-warning-800)] mb-3">{$t('critical.doctor_addressed')}</p>
          <div class="flex gap-2">
            <button
              class="flex-1 px-3 py-2 text-sm bg-[var(--color-warning-200)] text-[var(--color-warning-800)] rounded-lg
                     min-h-[44px] hover:bg-[var(--color-warning-200)]"
              onclick={handleConfirmStep}
            >
              {$t('critical.doctor_seen')}
            </button>
            <button
              class="px-3 py-2 text-sm bg-white dark:bg-gray-900 border border-[var(--color-warning-200)] text-[var(--color-warning-800)]
                     rounded-lg min-h-[44px] hover:bg-[var(--color-warning-50)]"
              onclick={cancelDismiss}
            >
              {$t('critical.not_yet')}
            </button>
          </div>
        {:else if dismissStep === 'reason'}
          <p class="text-sm text-[var(--color-warning-800)] mb-2">
            {$t('critical.confirm_addressed')}
          </p>
          <input
            type="text"
            class="w-full px-3 py-2 rounded-lg border border-[var(--color-warning-200)] text-stone-700 dark:text-gray-200
                   text-sm mb-2 min-h-[44px]"
            bind:value={dismissReason}
            placeholder={$t('critical.dismiss_reason_placeholder')}
            aria-label={$t('critical.dismiss_reason_label')}
          />
          {#if error}
            <p class="text-[var(--color-danger)] text-xs mb-2">{error}</p>
          {/if}
          <div class="flex gap-2">
            <button
              class="flex-1 px-3 py-2 text-sm bg-[var(--color-warning)] text-white rounded-lg
                     min-h-[44px] disabled:opacity-50"
              disabled={!dismissReason.trim()}
              onclick={handleDismissConfirm}
            >
              {$t('common.confirm')}
            </button>
            <button
              class="px-3 py-2 text-sm bg-white dark:bg-gray-900 border border-[var(--color-warning-200)] text-[var(--color-warning-800)]
                     rounded-lg min-h-[44px]"
              onclick={cancelDismiss}
            >
              {$t('common.cancel')}
            </button>
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/each}
