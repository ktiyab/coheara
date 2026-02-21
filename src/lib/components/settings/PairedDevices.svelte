<!-- ME-02: Paired Devices settings section -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { listPairedDevices, unpairDevice, getDeviceCount } from '$lib/api/devices';
  import type { DeviceSummary, DeviceCount } from '$lib/types/devices';
  import DeviceCard from './DeviceCard.svelte';
  import UnpairDialog from './UnpairDialog.svelte';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import ErrorState from '$lib/components/ui/ErrorState.svelte';

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

<section class="bg-white dark:bg-gray-900 rounded-xl p-5 border border-stone-100 dark:border-gray-800 shadow-sm">
  <div class="flex items-center justify-between mb-3">
    <h2 class="text-sm font-medium text-stone-500 dark:text-gray-400">{$t('devices.paired_heading')}</h2>
    <span class="text-xs text-stone-500 dark:text-gray-400">
      {$t('devices.paired_count', { values: { paired: counts.paired, max: counts.max } })} &middot; {$t('devices.connected_count', { values: { connected: counts.connected } })}
    </span>
  </div>

  {#if loading}
    <LoadingState variant="inline" message={$t('devices.loading')} />
  {:else if error}
    <ErrorState message={error} onretry={load} />
  {:else if devices.length === 0}
    <p class="text-sm text-stone-500 dark:text-gray-400 py-4 text-center">
      {$t('devices.no_devices')}
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
