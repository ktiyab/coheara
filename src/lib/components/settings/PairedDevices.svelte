<!-- UA02-12: Paired devices list with real-time status monitoring -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import { listPairedDevices, unpairDevice, getDeviceCount } from '$lib/api/devices';
  import type { DeviceSummary, DeviceCount } from '$lib/types/devices';
  import DeviceCard from './DeviceCard.svelte';
  import UnpairDialog from './UnpairDialog.svelte';
  import { PhoneIcon } from '$lib/components/icons/md';

  let devices = $state<DeviceSummary[]>([]);
  let counts = $state<DeviceCount>({ paired: 0, connected: 0, max: 3 });
  let loading = $state(true);
  let error: string | null = $state(null);
  let unpairTarget: DeviceSummary | null = $state(null);
  let refreshTimer: ReturnType<typeof setInterval> | null = null;

  async function load() {
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

  onMount(() => {
    load();
    // Auto-refresh every 5 seconds for real-time status
    refreshTimer = setInterval(load, 5000);
    return () => {
      if (refreshTimer) clearInterval(refreshTimer);
    };
  });

  const connectedCount = $derived(devices.filter(d => d.is_connected).length);
</script>

{#if loading && devices.length === 0}
  <!-- Skeleton -->
  <div class="animate-pulse space-y-3 px-4 py-3">
    {#each [1, 2] as _}
      <div class="flex items-center gap-3">
        <div class="w-10 h-10 rounded-full bg-stone-200 dark:bg-gray-700"></div>
        <div class="flex-1 space-y-2">
          <div class="h-3.5 w-32 bg-stone-200 dark:bg-gray-700 rounded"></div>
          <div class="h-3 w-48 bg-stone-200 dark:bg-gray-700 rounded"></div>
        </div>
      </div>
    {/each}
  </div>
{:else if error}
  <div class="px-4 py-6 text-center">
    <p class="text-sm text-red-500 dark:text-red-400 mb-2">{error}</p>
    <button
      class="text-sm text-[var(--color-interactive)] font-medium hover:underline"
      onclick={load}
    >
      {$t('common.try_again')}
    </button>
  </div>
{:else if devices.length === 0}
  <!-- Empty state -->
  <div class="px-4 py-8 text-center">
    <div class="w-12 h-12 mx-auto mb-3 rounded-full bg-stone-100 dark:bg-gray-800 flex items-center justify-center">
      <PhoneIcon class="w-6 h-6 text-stone-400 dark:text-gray-500" />
    </div>
    <p class="text-sm text-stone-500 dark:text-gray-400">{$t('devices.no_devices')}</p>
  </div>
{:else}
  <!-- Status header -->
  <div class="flex items-center justify-between px-4 py-2">
    <span class="text-xs font-medium text-stone-500 dark:text-gray-400 uppercase tracking-wider">
      {$t('devices.paired_heading')}
    </span>
    <div class="flex items-center gap-2 text-xs text-stone-500 dark:text-gray-400">
      <span>{$t('devices.paired_count', { values: { paired: counts.paired, max: counts.max } })}</span>
      <span class="text-stone-300 dark:text-gray-600">&middot;</span>
      <span class="flex items-center gap-1">
        <span class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
        {$t('devices.connected_count', { values: { connected: connectedCount } })}
      </span>
    </div>
  </div>

  <!-- Device list -->
  <div class="divide-y divide-stone-100 dark:divide-gray-800">
    {#each devices as device (device.device_id)}
      <DeviceCard {device} onUnpair={requestUnpair} />
    {/each}
  </div>
{/if}

{#if unpairTarget}
  <UnpairDialog
    deviceName={unpairTarget.device_name}
    onConfirm={confirmUnpair}
    onCancel={cancelUnpair}
  />
{/if}
