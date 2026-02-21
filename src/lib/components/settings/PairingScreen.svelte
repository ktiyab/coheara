<!-- M0-02: Device pairing screen with QR code display -->
<script lang="ts">
  import {
    startPairing,
    cancelPairing,
    getPendingApproval,
    approvePairing,
    denyPairing,
  } from '$lib/api/pairing';
  import { t } from 'svelte-i18n';
  import type { PairingStartResponse, PendingApproval } from '$lib/types/pairing';
  import ConfirmPairingDialog from './ConfirmPairingDialog.svelte';
  import Button from '$lib/components/ui/Button.svelte';

  type View = 'idle' | 'qr' | 'error';

  let view = $state<View>('idle');
  let pairingData: PairingStartResponse | null = $state(null);
  let pending: PendingApproval | null = $state(null);
  let error: string | null = $state(null);
  let loading = $state(false);
  let pollTimer: ReturnType<typeof setInterval> | null = $state(null);

  function timeRemaining(): string {
    if (!pairingData) return '';
    const expires = new Date(pairingData.expires_at).getTime();
    const now = Date.now();
    const secs = Math.max(0, Math.floor((expires - now) / 1000));
    const mins = Math.floor(secs / 60);
    const rem = secs % 60;
    return `${mins}:${rem.toString().padStart(2, '0')}`;
  }

  async function handleStart() {
    loading = true;
    error = null;
    try {
      pairingData = await startPairing();
      view = 'qr';
      startPolling();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      view = 'error';
    } finally {
      loading = false;
    }
  }

  async function handleCancel() {
    stopPolling();
    try {
      await cancelPairing();
    } catch {
      // Ignore cancel errors
    }
    pairingData = null;
    pending = null;
    view = 'idle';
  }

  function startPolling() {
    stopPolling();
    pollTimer = setInterval(async () => {
      try {
        const approval = await getPendingApproval();
        if (approval) {
          pending = approval;
          stopPolling();
        }
      } catch {
        // Polling errors are non-fatal
      }
    }, 1000);
  }

  function stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  async function handleApprove() {
    try {
      await approvePairing();
      pending = null;
      pairingData = null;
      view = 'idle';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      view = 'error';
    }
  }

  async function handleDeny() {
    try {
      await denyPairing();
    } catch {
      // Ignore
    }
    pending = null;
    pairingData = null;
    view = 'idle';
  }
</script>

<section class="bg-white dark:bg-gray-900 rounded-xl p-5 border border-stone-100 dark:border-gray-800 shadow-sm">
  <h2 class="text-sm font-medium text-stone-500 dark:text-gray-400 mb-3">{$t('pairing.pair_heading')}</h2>

  {#if view === 'idle'}
    <p class="text-sm text-stone-500 dark:text-gray-400 mb-4">
      {$t('pairing.scan_instruction')}
    </p>
    <Button variant="primary" fullWidth loading={loading} onclick={handleStart}>
      {loading ? $t('pairing.generating') : $t('pairing.generate_qr')}
    </Button>

  {:else if view === 'qr' && pairingData}
    <div class="qr-container">
      <!-- eslint-disable-next-line svelte/no-at-html-tags -->
      {@html pairingData.qr_svg}
    </div>
    <p class="text-sm text-stone-500 dark:text-gray-400 text-center mt-3">
      {$t('pairing.scan_qr_instruction')}
    </p>
    <p class="text-xs text-stone-500 dark:text-gray-400 text-center mt-1">
      {$t('pairing.same_wifi')}
    </p>
    <p class="text-xs text-stone-500 dark:text-gray-400 text-center mt-2">
      {$t('pairing.code_expires', { values: { time: timeRemaining() } })}
    </p>
    <div class="flex gap-3 mt-4">
      <Button variant="secondary" fullWidth onclick={handleCancel}>
        {$t('common.cancel')}
      </Button>
    </div>

  {:else if view === 'error'}
    <div class="bg-[var(--color-danger-50)] rounded-lg p-3 border border-[var(--color-danger-200)] mb-3">
      <p class="text-sm text-[var(--color-danger)]">{error}</p>
    </div>
    <button
      class="px-4 py-2 text-sm text-[var(--color-interactive)] border border-[var(--color-interactive)] rounded-lg"
      onclick={() => {
        view = 'idle';
        error = null;
      }}
    >
      {$t('common.try_again')}
    </button>
  {/if}
</section>

{#if pending}
  <ConfirmPairingDialog {pending} onApprove={handleApprove} onDeny={handleDeny} />
{/if}

<style>
  .qr-container {
    display: flex;
    justify-content: center;
    padding: 1rem;
    background: white;
    border-radius: 0.5rem;
    border: 1px solid var(--border-color, #e2e8f0);
  }
  .qr-container :global(svg) {
    width: 240px;
    height: 240px;
  }
</style>
