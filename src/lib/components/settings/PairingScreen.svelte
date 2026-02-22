<!-- UA02-12: Companion Devices hub — WhatsApp Linked Devices pattern -->
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
  import PairedDevices from './PairedDevices.svelte';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { ChevronLeftIcon, PlusIcon, PhoneIcon } from '$lib/components/icons/md';
  import Button from '$lib/components/ui/Button.svelte';

  type PairView = 'idle' | 'qr' | 'error';

  let pairView = $state<PairView>('idle');
  let pairingData: PairingStartResponse | null = $state(null);
  let pending: PendingApproval | null = $state(null);
  let error: string | null = $state(null);
  let loading = $state(false);
  let pollTimer: ReturnType<typeof setInterval> | null = $state(null);
  let countdownText = $state('');
  let countdownTimer: ReturnType<typeof setInterval> | null = null;

  function updateCountdown() {
    if (!pairingData) { countdownText = ''; return; }
    const expires = new Date(pairingData.expires_at).getTime();
    const now = Date.now();
    const secs = Math.max(0, Math.floor((expires - now) / 1000));
    if (secs === 0) { handleCancel(); return; }
    const mins = Math.floor(secs / 60);
    const rem = secs % 60;
    countdownText = `${mins}:${rem.toString().padStart(2, '0')}`;
  }

  async function handleStart() {
    loading = true;
    error = null;
    try {
      pairingData = await startPairing();
      pairView = 'qr';
      startPolling();
      countdownTimer = setInterval(updateCountdown, 1000);
      updateCountdown();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      pairView = 'error';
    } finally {
      loading = false;
    }
  }

  async function handleCancel() {
    stopPolling();
    if (countdownTimer) { clearInterval(countdownTimer); countdownTimer = null; }
    try { await cancelPairing(); } catch { /* ignore */ }
    pairingData = null;
    pending = null;
    pairView = 'idle';
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
      } catch { /* non-fatal */ }
    }, 1000);
  }

  function stopPolling() {
    if (pollTimer) { clearInterval(pollTimer); pollTimer = null; }
  }

  async function handleApprove() {
    try {
      await approvePairing();
      pending = null;
      pairingData = null;
      pairView = 'idle';
      if (countdownTimer) { clearInterval(countdownTimer); countdownTimer = null; }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      pairView = 'error';
    }
  }

  async function handleDeny() {
    try { await denyPairing(); } catch { /* ignore */ }
    pending = null;
    pairingData = null;
    pairView = 'idle';
    if (countdownTimer) { clearInterval(countdownTimer); countdownTimer = null; }
  }
</script>

<div class="flex flex-col bg-stone-50 dark:bg-gray-950 min-h-full">
  <!-- Header with back button -->
  <header class="flex items-center gap-3 px-4 py-3 bg-white dark:bg-gray-900 border-b border-stone-200 dark:border-gray-700">
    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center text-stone-500 dark:text-gray-400
             hover:text-stone-700 dark:hover:text-gray-200"
      onclick={() => navigation.navigate('settings')}
      aria-label={$t('nav.go_back')}
    >
      <ChevronLeftIcon class="w-5 h-5" />
    </button>
    <div class="flex items-center gap-2 flex-1">
      <PhoneIcon class="w-5 h-5 text-stone-400 dark:text-gray-500" />
      <h1 class="text-base font-medium text-stone-800 dark:text-gray-100">{$t('settings.hub_devices_title')}</h1>
    </div>
  </header>

  <div class="flex-1 overflow-y-auto">
    <div class="max-w-lg mx-auto pb-6">
      <!-- Section 1: Paired Devices List (real-time monitoring) -->
      <div class="mt-4 mx-4 bg-white dark:bg-gray-900 rounded-xl border border-stone-100 dark:border-gray-800 shadow-sm overflow-hidden">
        <PairedDevices />
      </div>

      <!-- Section 2: Pair New Device -->
      <div class="mt-4 mx-4 bg-white dark:bg-gray-900 rounded-xl border border-stone-100 dark:border-gray-800 shadow-sm p-5">
        <h2 class="text-xs font-medium text-stone-500 dark:text-gray-400 uppercase tracking-wider mb-3">
          {$t('pairing.pair_heading')}
        </h2>

        {#if pairView === 'idle'}
          <p class="text-sm text-stone-500 dark:text-gray-400 mb-4">
            {$t('pairing.scan_instruction')}
          </p>
          <Button variant="primary" fullWidth {loading} onclick={handleStart}>
            <span class="flex items-center justify-center gap-2">
              <PlusIcon class="w-5 h-5" />
              {loading ? $t('pairing.generating') : $t('pairing.generate_qr')}
            </span>
          </Button>

        {:else if pairView === 'qr' && pairingData}
          <!-- QR Code display -->
          <div class="flex justify-center p-4 bg-white rounded-lg border border-stone-200 dark:border-gray-700">
            <!-- eslint-disable-next-line svelte/no-at-html-tags -->
            <div class="[&>svg]:w-[200px] [&>svg]:h-[200px]">
              {@html pairingData.qr_svg}
            </div>
          </div>
          <p class="text-sm text-stone-500 dark:text-gray-400 text-center mt-3">
            {$t('pairing.scan_qr_instruction')}
          </p>
          <p class="text-xs text-stone-400 dark:text-gray-500 text-center mt-1">
            {$t('pairing.same_wifi')}
          </p>
          <p class="text-xs text-stone-400 dark:text-gray-500 text-center mt-2">
            {$t('pairing.code_expires', { values: { time: countdownText } })}
          </p>
          <div class="mt-4">
            <Button variant="secondary" fullWidth onclick={handleCancel}>
              {$t('common.cancel')}
            </Button>
          </div>

        {:else if pairView === 'error'}
          <div class="bg-red-50 dark:bg-red-900/20 rounded-lg p-3 border border-red-200 dark:border-red-800 mb-3">
            <p class="text-sm text-red-600 dark:text-red-400">{error}</p>
          </div>
          <button
            class="text-sm text-[var(--color-interactive)] font-medium hover:underline"
            onclick={() => { pairView = 'idle'; error = null; }}
          >
            {$t('common.try_again')}
          </button>
        {/if}
      </div>
    </div>
  </div>
</div>

{#if pending}
  <ConfirmPairingDialog {pending} onApprove={handleApprove} onDeny={handleDeny} />
{/if}
