<!-- ME-02: Paired Devices settings section -->
<script lang="ts">
  import { listPairedDevices, unpairDevice, getDeviceCount } from '$lib/api/devices';
  import type { DeviceSummary, DeviceCount } from '$lib/types/devices';
  import DeviceCard from './DeviceCard.svelte';
  import UnpairDialog from './UnpairDialog.svelte';

  let devices = $state<DeviceSummary[]>([]);
  let counts = $state<DeviceCount>({ paired: 0, connected: 0, max: 3 });
  let loading = $state(true);
  let error: string | null = $state(null);
  let unpairTarget: DeviceSummary | null = $state(null);

  async function load() {
    loading = true;
    error = null;
    try {
      const [d, c] = await Promise.all([listPairedDevices(), getDeviceCount()]);
      devices = d;
      counts = c;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function requestUnpair(deviceId: string) {
    unpairTarget = devices.find((d) => d.device_id === deviceId) ?? null;
  }

  async function confirmUnpair() {
    if (!unpairTarget) return;
    const id = unpairTarget.device_id;
    unpairTarget = null;
    try {
      await unpairDevice(id);
      await load();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function cancelUnpair() {
    unpairTarget = null;
  }

  $effect(() => {
    load();
  });
</script>

<section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
  <div class="flex items-center justify-between mb-3">
    <h2 class="text-sm font-medium text-stone-500">PAIRED DEVICES</h2>
    <span class="text-xs text-stone-400">
      {counts.paired}/{counts.max} paired &middot; {counts.connected} connected
    </span>
  </div>

  {#if loading}
    <p class="text-sm text-stone-400 py-4 text-center">Loading devices...</p>
  {:else if error}
    <div class="bg-red-50 rounded-lg p-3 border border-red-200 mb-3">
      <p class="text-sm text-red-700">{error}</p>
    </div>
    <button
      class="px-4 py-2 text-sm text-teal-600 border border-teal-200 rounded-lg"
      onclick={load}
    >
      Retry
    </button>
  {:else if devices.length === 0}
    <p class="text-sm text-stone-400 py-4 text-center">
      No devices paired. Use the Coheara mobile app to pair a phone via WiFi.
    </p>
  {:else}
    <div class="space-y-0">
      {#each devices as device (device.device_id)}
        <DeviceCard {device} onUnpair={requestUnpair} />
      {/each}
    </div>
  {/if}
</section>

{#if unpairTarget}
  <UnpairDialog
    deviceName={unpairTarget.device_name}
    onConfirm={confirmUnpair}
    onCancel={cancelUnpair}
  />
{/if}
