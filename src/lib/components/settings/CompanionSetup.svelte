<!-- ADS: Companion app setup — start distribution server, show QR, guide user -->
<script lang="ts">
  import {
    startDistribution,
    stopDistribution,
    getDistributionStatus,
  } from '$lib/api/distribution';
  import type { InstallQrCode, DistributionStatus } from '$lib/types/distribution';
  import Button from '$lib/components/ui/Button.svelte';

  type View = 'idle' | 'serving' | 'error';

  let view = $state<View>('idle');
  let qrCode: InstallQrCode | null = $state(null);
  let status: DistributionStatus | null = $state(null);
  let error: string | null = $state(null);
  let loading = $state(false);
  let statusTimer: ReturnType<typeof setInterval> | null = $state(null);

  async function handleStart() {
    loading = true;
    error = null;
    try {
      qrCode = await startDistribution();
      view = 'serving';
      startStatusPolling();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      view = 'error';
    } finally {
      loading = false;
    }
  }

  async function handleStop() {
    stopStatusPolling();
    try {
      await stopDistribution();
    } catch {
      // Ignore stop errors
    }
    qrCode = null;
    status = null;
    view = 'idle';
  }

  function startStatusPolling() {
    stopStatusPolling();
    statusTimer = setInterval(async () => {
      try {
        status = await getDistributionStatus();
      } catch {
        // Non-fatal
      }
    }, 3000);
  }

  function stopStatusPolling() {
    if (statusTimer) {
      clearInterval(statusTimer);
      statusTimer = null;
    }
  }
</script>

<section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
  <h2 class="text-sm font-medium text-stone-500 mb-3">INSTALL COMPANION APP</h2>

  {#if view === 'idle'}
    <p class="text-sm text-stone-500 mb-4">
      Serve the Coheara companion app to your phone over WiFi.
      No app store needed.
    </p>
    <Button variant="primary" fullWidth loading={loading} onclick={handleStart}>
      {loading ? 'Starting...' : 'Start Companion Server'}
    </Button>

  {:else if view === 'serving' && qrCode}
    <div class="qr-container">
      <!-- eslint-disable-next-line svelte/no-at-html-tags -->
      {@html qrCode.svg}
    </div>

    <p class="text-sm text-stone-700 text-center mt-3 font-medium">
      Scan with your phone camera
    </p>
    <p class="text-xs text-stone-500 text-center mt-1">
      Your phone will open a page to install the companion app.
    </p>

    <div class="mt-4 bg-stone-50 rounded-lg p-3 text-xs text-stone-500 space-y-1">
      <p><span class="font-medium">URL:</span> {qrCode.url}</p>
      <p><span class="font-medium">Version:</span> {qrCode.desktop_version}</p>
      {#if status}
        <p><span class="font-medium">Requests:</span> {status.request_count}</p>
        <p>
          <span class="font-medium">APK:</span>
          {status.apk_available ? 'Available' : 'Not bundled'}
          &middot;
          <span class="font-medium">PWA:</span>
          {status.pwa_available ? 'Available' : 'Not bundled'}
        </p>
      {/if}
    </div>

    <div class="flex gap-3 mt-4">
      <Button variant="secondary" fullWidth onclick={handleStop}>
        Stop Server
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
      Try again
    </button>
  {/if}
</section>

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
