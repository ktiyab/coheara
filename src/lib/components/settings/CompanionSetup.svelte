<!-- ADS: Companion app setup — start distribution server, show QR, guide user -->
<script lang="ts">
  import { t } from 'svelte-i18n';
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

<section class="bg-white dark:bg-gray-900 rounded-xl p-5 border border-stone-100 dark:border-gray-800 shadow-sm">
  <h2 class="text-sm font-medium text-stone-500 dark:text-gray-400 mb-3">{$t('companion.heading')}</h2>

  {#if view === 'idle'}
    <p class="text-sm text-stone-500 dark:text-gray-400 mb-4">
      {$t('companion.description_line1')}
      {$t('companion.description_line2')}
    </p>
    <Button variant="primary" fullWidth loading={loading} onclick={handleStart}>
      {loading ? $t('companion.starting') : $t('companion.start_server')}
    </Button>

  {:else if view === 'serving' && qrCode}
    <div class="qr-container">
      <!-- eslint-disable-next-line svelte/no-at-html-tags -->
      {@html qrCode.svg}
    </div>

    <p class="text-sm text-stone-700 dark:text-gray-200 text-center mt-3 font-medium">
      {$t('companion.scan_camera')}
    </p>
    <p class="text-xs text-stone-500 dark:text-gray-400 text-center mt-1">
      {$t('companion.phone_install_note')}
    </p>

    <div class="mt-4 bg-stone-50 dark:bg-gray-950 rounded-lg p-3 text-xs text-stone-500 dark:text-gray-400 space-y-1">
      <p><span class="font-medium">{$t('companion.url_label')}</span> {qrCode.url}</p>
      <p><span class="font-medium">{$t('companion.version_label')}</span> {qrCode.desktop_version}</p>
      {#if status}
        <p><span class="font-medium">{$t('companion.requests_label')}</span> {status.request_count}</p>
        <p>
          <span class="font-medium">{$t('companion.apk_label')}</span>
          {status.apk_available ? $t('companion.available') : $t('companion.not_bundled')}
          &middot;
          <span class="font-medium">{$t('companion.pwa_label')}</span>
          {status.pwa_available ? $t('companion.available') : $t('companion.not_bundled')}
        </p>
      {/if}
    </div>

    <div class="flex gap-3 mt-4">
      <Button variant="secondary" fullWidth onclick={handleStop}>
        {$t('companion.stop_server')}
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
