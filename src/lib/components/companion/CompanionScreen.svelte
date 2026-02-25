<!-- CP-01: Unified Companion Screen — WhatsApp Linked Devices + Google Home pattern -->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { t } from 'svelte-i18n';
  import {
    startDistribution,
    stopDistribution,
    getDistributionStatus,
  } from '$lib/api/distribution';
  import {
    startMobileApi,
    stopMobileApi,
    getMobileApiStatus,
  } from '$lib/api/mobile_api';
  import {
    startPairing,
    cancelPairing,
    getPendingApproval,
    approvePairing,
    denyPairing,
  } from '$lib/api/pairing';
  import type { InstallQrCode, DistributionStatus } from '$lib/types/distribution';
  import type { MobileApiSession } from '$lib/types/mobile_api';
  import type { PairingStartResponse, PendingApproval } from '$lib/types/pairing';
  import PairedDevices from '$lib/components/settings/PairedDevices.svelte';
  import ConfirmPairingDialog from '$lib/components/settings/ConfirmPairingDialog.svelte';
  import { DevicesIcon, LockIcon, PlusIcon } from '$lib/components/icons/md';
  import Button from '$lib/components/ui/Button.svelte';

  // ═══ Zone B: Distribution server state ═══
  type ServerView = 'idle' | 'starting' | 'serving' | 'error';
  let serverView = $state<ServerView>('idle');
  let qrCode: InstallQrCode | null = $state(null);
  let distStatus: DistributionStatus | null = $state(null);
  let serverError: string | null = $state(null);
  let serverLoading = $state(false);
  let statusTimer: ReturnType<typeof setInterval> | null = null;

  async function handleStartServer() {
    serverLoading = true;
    serverError = null;
    serverView = 'starting';
    try {
      qrCode = await startDistribution();
      serverView = 'serving';
      startStatusPolling();
    } catch (e) {
      serverError = e instanceof Error ? e.message : String(e);
      serverView = 'error';
    } finally {
      serverLoading = false;
    }
  }

  async function handleStopServer() {
    stopStatusPolling();
    try { await stopDistribution(); } catch { /* ignore */ }
    qrCode = null;
    distStatus = null;
    serverView = 'idle';
  }

  function startStatusPolling() {
    stopStatusPolling();
    statusTimer = setInterval(async () => {
      try { distStatus = await getDistributionStatus(); } catch { /* non-fatal */ }
    }, 3000);
  }

  function stopStatusPolling() {
    if (statusTimer) { clearInterval(statusTimer); statusTimer = null; }
  }

  // ═══ Zone B2: HTTPS API server state (SEC-HTTPS-01) ═══
  type ApiServerView = 'idle' | 'starting' | 'running' | 'error';
  let apiServerView = $state<ApiServerView>('idle');
  let apiSession: MobileApiSession | null = $state(null);
  let apiError: string | null = $state(null);
  let apiLoading = $state(false);

  async function handleStartApiServer() {
    apiLoading = true;
    apiError = null;
    apiServerView = 'starting';
    try {
      apiSession = await startMobileApi();
      apiServerView = 'running';
    } catch (e) {
      apiError = e instanceof Error ? e.message : String(e);
      apiServerView = 'error';
    } finally {
      apiLoading = false;
    }
  }

  async function handleStopApiServer() {
    try { await stopMobileApi(); } catch { /* ignore */ }
    apiSession = null;
    apiServerView = 'idle';
  }

  // ═══ On-mount: detect already-running servers ═══
  onMount(async () => {
    try {
      const [apiStatus, distStatusResult] = await Promise.all([
        getMobileApiStatus(),
        getDistributionStatus(),
      ]);
      if (apiStatus?.running && apiStatus.session) {
        apiSession = apiStatus.session;
        apiServerView = 'running';
      }
      if (distStatusResult?.session) {
        distStatus = distStatusResult;
        serverView = 'serving';
        startStatusPolling();
      }
    } catch { /* non-fatal — stay idle */ }
  });

  // ═══ Zone D: Pairing flow state ═══
  let showPairing = $state(false);
  let pairingData: PairingStartResponse | null = $state(null);
  let pendingApproval: PendingApproval | null = $state(null);
  let pairingError: string | null = $state(null);
  let pairingLoading = $state(false);
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let countdownText = $state('');
  let countdownTimer: ReturnType<typeof setInterval> | null = null;

  function updateCountdown() {
    if (!pairingData) { countdownText = ''; return; }
    const expires = new Date(pairingData.expires_at).getTime();
    const now = Date.now();
    const secs = Math.max(0, Math.floor((expires - now) / 1000));
    if (secs === 0) { handleCancelPairing(); return; }
    const mins = Math.floor(secs / 60);
    const rem = secs % 60;
    countdownText = `${mins}:${rem.toString().padStart(2, '0')}`;
  }

  async function handleStartPairing() {
    pairingLoading = true;
    pairingError = null;
    showPairing = true;
    try {
      pairingData = await startPairing();
      startApprovalPolling();
      countdownTimer = setInterval(updateCountdown, 1000);
      updateCountdown();
    } catch (e) {
      pairingError = e instanceof Error ? e.message : String(e);
    } finally {
      pairingLoading = false;
    }
  }

  async function handleCancelPairing() {
    stopApprovalPolling();
    if (countdownTimer) { clearInterval(countdownTimer); countdownTimer = null; }
    try { await cancelPairing(); } catch { /* ignore */ }
    pairingData = null;
    pendingApproval = null;
    pairingError = null;
    showPairing = false;
  }

  function startApprovalPolling() {
    stopApprovalPolling();
    pollTimer = setInterval(async () => {
      try {
        const approval = await getPendingApproval();
        if (approval) {
          pendingApproval = approval;
          stopApprovalPolling();
        }
      } catch { /* non-fatal */ }
    }, 1000);
  }

  function stopApprovalPolling() {
    if (pollTimer) { clearInterval(pollTimer); pollTimer = null; }
  }

  async function handleApprove() {
    try {
      await approvePairing();
      pendingApproval = null;
      pairingData = null;
      showPairing = false;
      if (countdownTimer) { clearInterval(countdownTimer); countdownTimer = null; }
    } catch (e) {
      pairingError = e instanceof Error ? e.message : String(e);
    }
  }

  async function handleDeny() {
    try { await denyPairing(); } catch { /* ignore */ }
    pendingApproval = null;
    pairingData = null;
    showPairing = false;
    if (countdownTimer) { clearInterval(countdownTimer); countdownTimer = null; }
  }

  // ═══ Cleanup ═══
  onDestroy(() => {
    stopStatusPolling();
    stopApprovalPolling();
    if (countdownTimer) clearInterval(countdownTimer);
  });
</script>

<div class="flex flex-col bg-stone-50 dark:bg-gray-950 min-h-full">
  <!-- Header -->
  <header class="flex items-center gap-3 px-4 py-3 bg-stone-50 dark:bg-gray-950">
    <DevicesIcon class="w-5 h-5 text-[var(--color-success)]" />
    <h1 class="text-base font-medium text-stone-800 dark:text-gray-100">{$t('companion.screen_title')}</h1>
  </header>

  <div class="flex-1 overflow-y-auto">
    <div class="max-w-lg mx-auto pb-6">

      <!-- ═══ Zone A: What is the Companion? ═══ -->
      <section class="mt-4 mx-4 bg-white dark:bg-gray-900 rounded-xl border border-stone-100 dark:border-gray-800 shadow-sm p-5">
        <h2 class="text-sm font-semibold text-[var(--color-success)] mb-2">
          {$t('companion.what_heading')}
        </h2>
        <p class="text-sm text-stone-500 dark:text-gray-400 leading-relaxed mb-4">
          {$t('companion.what_description')}
        </p>
        <div class="flex flex-col gap-2">
          <div class="flex items-center gap-2 text-xs text-stone-500 dark:text-gray-400">
            <LockIcon class="w-4 h-4 text-emerald-500 flex-shrink-0" />
            <span>{$t('companion.trust_privacy')}</span>
          </div>
          <div class="flex items-center gap-2 text-xs text-stone-500 dark:text-gray-400">
            <DevicesIcon class="w-4 h-4 text-[var(--color-success)] flex-shrink-0" />
            <span>{$t('companion.trust_wifi')}</span>
          </div>
          <div class="flex items-center gap-2 text-xs text-stone-500 dark:text-gray-400">
            <LockIcon class="w-4 h-4 text-[var(--color-success)] flex-shrink-0" />
            <span>{$t('companion.trust_encrypted')}</span>
          </div>
        </div>
      </section>

      <!-- ═══ Zone B: Get the App (Distribution Server) ═══ -->
      <section class="mt-4 mx-4 bg-white dark:bg-gray-900 rounded-xl border border-stone-100 dark:border-gray-800 shadow-sm p-5">
        <h2 class="text-xs font-medium text-[var(--color-success)] uppercase tracking-wider mb-3">
          {$t('companion.get_app_heading')}
        </h2>

        {#if serverView === 'idle'}
          <p class="text-sm text-stone-500 dark:text-gray-400 mb-4">
            {$t('companion.get_app_description')}
          </p>
          <Button variant="primary" fullWidth loading={serverLoading} onclick={handleStartServer}>
            {$t('companion.start_server')}
          </Button>

        {:else if serverView === 'starting'}
          <div class="flex items-center justify-center gap-2 py-6">
            <div class="w-5 h-5 border-2 border-[var(--color-success)] border-t-transparent rounded-full animate-spin"></div>
            <span class="text-sm text-stone-500 dark:text-gray-400">{$t('companion.starting')}</span>
          </div>

        {:else if serverView === 'serving' && qrCode}
          <!-- QR code -->
          <div class="flex justify-center p-4 bg-white rounded-lg border border-stone-200 dark:border-gray-700 [&>svg]:w-[220px] [&>svg]:h-[220px]">
            <!-- eslint-disable-next-line svelte/no-at-html-tags -->
            {@html qrCode.svg}
          </div>

          <p class="text-sm text-stone-700 dark:text-gray-200 text-center mt-3 font-medium">
            {$t('companion.scan_camera')}
          </p>
          <p class="text-xs text-stone-500 dark:text-gray-400 text-center mt-1">
            {$t('companion.get_app_note')}
          </p>

          <!-- Server info -->
          <div class="mt-4 bg-stone-50 dark:bg-gray-950 rounded-lg p-3 text-xs text-stone-500 dark:text-gray-400 space-y-1">
            <div class="flex items-center gap-1.5">
              <span class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
              <span class="font-medium">{$t('companion.server_active')}</span>
            </div>
            <p><span class="font-medium">{$t('companion.url_label')}</span> {qrCode.url}</p>
            <p><span class="font-medium">{$t('companion.version_label')}</span> {qrCode.desktop_version}</p>
            {#if distStatus}
              <p><span class="font-medium">{$t('companion.requests_label')}</span> {distStatus.request_count}</p>
              <p>
                <span class="font-medium">{$t('companion.apk_label')}</span>
                {distStatus.apk_available ? $t('companion.available') : $t('companion.not_bundled')}
                &middot;
                <span class="font-medium">{$t('companion.pwa_label')}</span>
                {distStatus.pwa_available ? $t('companion.available') : $t('companion.not_bundled')}
              </p>
            {/if}
          </div>

          <div class="mt-4">
            <Button variant="secondary" fullWidth onclick={handleStopServer}>
              {$t('companion.stop_server')}
            </Button>
          </div>

        {:else if serverView === 'error'}
          <div class="bg-red-50 dark:bg-red-900/20 rounded-lg p-3 border border-red-200 dark:border-red-800 mb-3">
            <p class="text-sm text-red-600 dark:text-red-400">{serverError}</p>
          </div>
          <button
            class="text-sm text-[var(--color-success)] font-medium hover:underline"
            onclick={() => { serverView = 'idle'; serverError = null; }}
          >
            {$t('common.try_again')}
          </button>
        {/if}
      </section>

      <!-- ═══ Zone B2: Secure Connection (HTTPS API Server) ═══ -->
      <section class="mt-4 mx-4 bg-white dark:bg-gray-900 rounded-xl border border-stone-100 dark:border-gray-800 shadow-sm p-5">
        <h2 class="text-xs font-medium text-[var(--color-success)] uppercase tracking-wider mb-3">
          {$t('companion.secure_server_heading')}
        </h2>

        {#if apiServerView === 'idle'}
          <p class="text-sm text-stone-500 dark:text-gray-400 mb-4">
            {$t('companion.secure_server_description')}
          </p>
          <Button variant="primary" fullWidth loading={apiLoading} onclick={handleStartApiServer}>
            <span class="flex items-center justify-center gap-2">
              <LockIcon class="w-4 h-4" />
              {$t('companion.secure_server_start')}
            </span>
          </Button>

        {:else if apiServerView === 'starting'}
          <div class="flex items-center justify-center gap-2 py-6">
            <div class="w-5 h-5 border-2 border-[var(--color-success)] border-t-transparent rounded-full animate-spin"></div>
            <span class="text-sm text-stone-500 dark:text-gray-400">{$t('companion.secure_server_starting')}</span>
          </div>

        {:else if apiServerView === 'running' && apiSession}
          <div class="bg-stone-50 dark:bg-gray-950 rounded-lg p-3 text-xs text-stone-500 dark:text-gray-400 space-y-1">
            <div class="flex items-center gap-1.5">
              <span class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
              <span class="font-medium">{$t('companion.secure_server_running')}</span>
            </div>
            <p><span class="font-medium">{$t('companion.secure_server_address')}</span> {apiSession.server_addr}</p>
            <p><span class="font-medium">{$t('companion.secure_server_port')}</span> {apiSession.port}</p>
          </div>
          <div class="mt-4">
            <Button variant="secondary" fullWidth onclick={handleStopApiServer}>
              {$t('companion.secure_server_stop')}
            </Button>
          </div>

        {:else if apiServerView === 'error'}
          <div class="bg-red-50 dark:bg-red-900/20 rounded-lg p-3 border border-red-200 dark:border-red-800 mb-3">
            <p class="text-sm text-red-600 dark:text-red-400">{apiError}</p>
          </div>
          <button
            class="text-sm text-[var(--color-success)] font-medium hover:underline"
            onclick={() => { apiServerView = 'idle'; apiError = null; }}
          >
            {$t('common.try_again')}
          </button>
        {/if}
      </section>

      <!-- ═══ Zone C: Paired Devices ═══ -->
      <section class="mt-4 mx-4 bg-white dark:bg-gray-900 rounded-xl border border-stone-100 dark:border-gray-800 shadow-sm overflow-hidden">
        <div class="px-4 pt-4 pb-2 flex items-center justify-between">
          <h2 class="text-xs font-medium text-[var(--color-success)] uppercase tracking-wider">
            {$t('companion.paired_heading')}
          </h2>
        </div>
        <PairedDevices />
      </section>

      <!-- ═══ Zone D: Pair New Device ═══ -->
      <section class="mt-4 mx-4 bg-white dark:bg-gray-900 rounded-xl border border-stone-100 dark:border-gray-800 shadow-sm p-5">
        {#if !showPairing}
          <p class="text-sm text-stone-500 dark:text-gray-400 mb-4">
            {$t('companion.pair_description')}
          </p>
          {#if apiServerView !== 'running'}
            <p class="text-xs text-amber-600 dark:text-amber-400 mb-3">
              {$t('companion.secure_server_required')}
            </p>
          {/if}
          <Button variant="primary" fullWidth disabled={apiServerView !== 'running'} onclick={handleStartPairing}>
            <span class="flex items-center justify-center gap-2">
              <PlusIcon class="w-5 h-5" />
              {$t('companion.pair_new')}
            </span>
          </Button>

        {:else if pairingError}
          <div class="bg-red-50 dark:bg-red-900/20 rounded-lg p-3 border border-red-200 dark:border-red-800 mb-3">
            <p class="text-sm text-red-600 dark:text-red-400">{pairingError}</p>
          </div>
          <button
            class="text-sm text-[var(--color-success)] font-medium hover:underline"
            onclick={() => { pairingError = null; showPairing = false; }}
          >
            {$t('common.try_again')}
          </button>

        {:else if pairingLoading}
          <div class="flex items-center justify-center gap-2 py-6">
            <div class="w-5 h-5 border-2 border-[var(--color-success)] border-t-transparent rounded-full animate-spin"></div>
            <span class="text-sm text-stone-500 dark:text-gray-400">{$t('pairing.generating')}</span>
          </div>

        {:else if pairingData}
          <!-- Pairing QR code -->
          <div class="flex justify-center p-4 bg-white rounded-lg border border-stone-200 dark:border-gray-700">
            <div class="[&>svg]:w-[200px] [&>svg]:h-[200px]">
              <!-- eslint-disable-next-line svelte/no-at-html-tags -->
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
            <Button variant="secondary" fullWidth onclick={handleCancelPairing}>
              {$t('companion.pair_cancel')}
            </Button>
          </div>
        {/if}
      </section>

    </div>
  </div>
</div>

<!-- Approval dialog (floats above everything) -->
{#if pendingApproval}
  <ConfirmPairingDialog pending={pendingApproval} onApprove={handleApprove} onDeny={handleDeny} />
{/if}
