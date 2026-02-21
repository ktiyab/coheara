<!-- L4-03: WiFi Transfer screen — QR code display, PIN, received files list. -->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    startWifiTransfer, stopWifiTransfer,
    getTransferStatus, processStagedFiles
  } from '$lib/api/transfer';
  import type { QrCodeData, UploadResult, TransferStatus } from '$lib/types/transfer';
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import ErrorState from '$lib/components/ui/ErrorState.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import { CheckIcon } from '$lib/components/icons/md';

  let status: TransferStatus = $state('starting');
  let qrData: QrCodeData | null = $state(null);
  let receivedFiles: UploadResult[] = $state([]);
  let error: string | null = $state(null);
  let pollInterval: ReturnType<typeof setInterval> | null = $state(null);

  onMount(async () => {
    try {
      qrData = await startWifiTransfer();
      status = 'active';
      // Poll for received files every 3 seconds
      pollInterval = setInterval(async () => {
        try {
          const resp = await getTransferStatus();
          if (resp) {
            receivedFiles = resp.received_files;
          }
        } catch {
          // Server may have auto-shutdown
          status = 'idle';
          if (pollInterval) clearInterval(pollInterval);
        }
      }, 3000);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      status = 'error';
    }
  });

  onDestroy(() => {
    if (pollInterval) clearInterval(pollInterval);
    // Fire-and-forget stop — component is being destroyed
    stopWifiTransfer().catch(() => {});
  });

  async function handleDone() {
    status = 'stopping';
    if (pollInterval) clearInterval(pollInterval);
    try {
      await stopWifiTransfer();
      const count = await processStagedFiles();
      if (count > 0) {
        // Files were imported — home screen should refresh
      }
      navigation.navigate('home');
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      status = 'error';
    }
  }
</script>

<div class="flex flex-col items-center bg-stone-50 dark:bg-gray-950 px-6 py-8">
  {#if status === 'starting'}
    <LoadingState message={$t('transfer.starting_server')} />

  {:else if status === 'error'}
    <ErrorState
      message={error ?? ''}
      onretry={() => navigation.goBack()}
      retryLabel={$t('common.go_back')}
    />

  {:else if status === 'active' && qrData}
    <h1 class="text-xl font-semibold text-stone-800 dark:text-gray-100 mb-2">{$t('transfer.receive_heading')}</h1>
    <p class="text-sm text-stone-500 dark:text-gray-400 mb-6 text-center">
      {$t('transfer.scan_instruction')}
    </p>

    <!-- QR Code -->
    <div class="bg-white dark:bg-gray-900 p-6 rounded-2xl shadow-sm border border-stone-100 dark:border-gray-800 mb-6">
      {@html qrData.svg}
    </div>

    <!-- PIN display -->
    <div class="mb-4 text-center">
      <p class="text-xs text-stone-500 dark:text-gray-400 mb-1">{$t('transfer.enter_pin')}</p>
      <p
        class="text-4xl font-mono font-bold tracking-[0.3em] text-stone-800 dark:text-gray-100"
        aria-label={$t('transfer.pin_aria', { values: { pin: qrData.pin } })}
      >
        {qrData.pin}
      </p>
    </div>

    <!-- URL fallback -->
    <p class="text-xs text-stone-500 dark:text-gray-400 mb-8 text-center">
      {$t('transfer.url_fallback')}<br>
      <span class="font-mono text-stone-500 dark:text-gray-400">{qrData.url}</span>
    </p>

    <!-- Received files -->
    {#if receivedFiles.length > 0}
      <div class="w-full max-w-sm mb-6">
        <h2 class="text-sm font-medium text-stone-600 dark:text-gray-300 mb-2">
          {$t('transfer.files_received', { values: { count: receivedFiles.length } })}
        </h2>
        {#each receivedFiles as file}
          <div class="flex items-center gap-3 py-2 px-3 bg-[var(--color-success-50)] rounded-lg mb-1">
            <span class="text-[var(--color-success)] text-sm"><CheckIcon class="w-3.5 h-3.5" /></span>
            <span class="text-sm text-stone-700 dark:text-gray-200 truncate">{file.filename}</span>
            <span class="text-xs text-stone-500 dark:text-gray-400 ml-auto">
              {Math.round(file.size_bytes / 1024)}KB
            </span>
          </div>
        {/each}
      </div>
    {/if}

    <!-- Done button -->
    <div class="w-full max-w-sm flex flex-col gap-2">
      <Button variant="primary" fullWidth onclick={handleDone}>
        {$t('transfer.done_button')}
      </Button>
      <Button variant="ghost" fullWidth onclick={() => navigation.goBack()}>
        {$t('common.cancel')}
      </Button>
    </div>

  {:else if status === 'stopping'}
    <LoadingState message={$t('transfer.processing_files')} />

  {:else}
    <!-- idle / server stopped -->
    <div class="flex flex-col items-center justify-center flex-1">
      <p class="text-stone-500 dark:text-gray-400 mb-4">{$t('transfer.session_ended')}</p>
      <Button variant="secondary" onclick={() => navigation.goBack()}>
        {$t('common.go_back')}
      </Button>
    </div>
  {/if}
</div>
