<!-- L4-03: WiFi Transfer screen — QR code display, PIN, received files list. -->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    startWifiTransfer, stopWifiTransfer,
    getTransferStatus, processStagedFiles
  } from '$lib/api/transfer';
  import type { QrCodeData, UploadResult, TransferStatus } from '$lib/types/transfer';
  import { navigation } from '$lib/stores/navigation.svelte';

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

<div class="flex flex-col items-center min-h-screen pb-20 bg-stone-50 px-6 py-8">
  {#if status === 'starting'}
    <div class="flex flex-col items-center justify-center flex-1">
      <div class="animate-spin w-8 h-8 border-2 border-[var(--color-primary)]
                  border-t-transparent rounded-full mb-4"></div>
      <p class="text-stone-500">Starting transfer server...</p>
    </div>

  {:else if status === 'error'}
    <div class="flex flex-col items-center justify-center flex-1">
      <p class="text-red-600 mb-4">{error}</p>
      <button
        class="px-6 py-3 bg-stone-200 rounded-xl text-stone-700 min-h-[44px]"
        onclick={() => navigation.goBack()}
      >
        Go back
      </button>
    </div>

  {:else if status === 'active' && qrData}
    <h2 class="text-xl font-semibold text-stone-800 mb-2">Receive from phone</h2>
    <p class="text-sm text-stone-500 mb-6 text-center">
      Scan this code with your phone camera to send documents.
    </p>

    <!-- QR Code -->
    <div class="bg-white p-6 rounded-2xl shadow-sm border border-stone-100 mb-6">
      {@html qrData.svg}
    </div>

    <!-- PIN display -->
    <div class="mb-4 text-center">
      <p class="text-xs text-stone-500 mb-1">Enter this PIN on your phone:</p>
      <p class="text-4xl font-mono font-bold tracking-[0.3em] text-stone-800">
        {qrData.pin}
      </p>
    </div>

    <!-- URL fallback -->
    <p class="text-xs text-stone-400 mb-8 text-center">
      Or type this in your phone's browser:<br>
      <span class="font-mono text-stone-500">{qrData.url}</span>
    </p>

    <!-- Received files -->
    {#if receivedFiles.length > 0}
      <div class="w-full max-w-sm mb-6">
        <h3 class="text-sm font-medium text-stone-600 mb-2">
          {receivedFiles.length} file{receivedFiles.length === 1 ? '' : 's'} received
        </h3>
        {#each receivedFiles as file}
          <div class="flex items-center gap-3 py-2 px-3 bg-green-50 rounded-lg mb-1">
            <span class="text-green-600 text-sm">&#x2713;</span>
            <span class="text-sm text-stone-700 truncate">{file.filename}</span>
            <span class="text-xs text-stone-400 ml-auto">
              {Math.round(file.size_bytes / 1024)}KB
            </span>
          </div>
        {/each}
      </div>
    {/if}

    <!-- Done button -->
    <button
      class="w-full max-w-sm px-6 py-4 bg-[var(--color-primary)] text-white rounded-xl
             text-base font-medium min-h-[44px]"
      onclick={handleDone}
    >
      Done receiving
    </button>
    <button
      class="mt-2 text-stone-500 text-sm min-h-[44px]"
      onclick={() => navigation.goBack()}
    >
      Cancel
    </button>

  {:else if status === 'stopping'}
    <div class="flex flex-col items-center justify-center flex-1">
      <div class="animate-pulse text-stone-400">Processing received files...</div>
    </div>

  {:else}
    <!-- idle / server stopped -->
    <div class="flex flex-col items-center justify-center flex-1">
      <p class="text-stone-500 mb-4">Transfer session ended.</p>
      <button
        class="px-6 py-3 bg-stone-200 rounded-xl text-stone-700 min-h-[44px]"
        onclick={() => navigation.goBack()}
      >
        Go back
      </button>
    </div>
  {/if}
</div>
