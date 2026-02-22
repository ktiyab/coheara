<!-- UA02-12: Device card — WhatsApp Linked Devices style -->
<script lang="ts">
  import { t, locale } from 'svelte-i18n';
  import type { DeviceSummary } from '$lib/types/devices';
  import { PhoneIcon, DeleteIcon, WarningIcon } from '$lib/components/icons/md';

  let { device, onUnpair }: { device: DeviceSummary; onUnpair: (id: string) => void } = $props();

  function formatDate(iso: string): string {
    return new Date(iso).toLocaleDateString($locale ?? undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  }

  function lastSeenText(): string {
    if (device.is_connected) return $t('devices.status_connected');
    if (!device.days_inactive || device.days_inactive === 0) return $t('devices.last_seen_just_now');
    if (device.days_inactive === 1) return $t('devices.last_seen_yesterday');
    return $t('devices.last_seen_days_ago', { values: { days: device.days_inactive } });
  }

  const isInactive = $derived((device.days_inactive ?? 0) >= 30);
</script>

<div
  class="group relative flex items-center gap-3 px-4 py-3 min-h-[60px]
         hover:bg-stone-50 dark:hover:bg-gray-800/50 transition-colors
         {isInactive ? 'border-l-2 border-l-amber-400' : ''}"
>
  <!-- Device icon with status dot -->
  <div class="relative flex-shrink-0">
    <div class="w-10 h-10 rounded-full bg-stone-100 dark:bg-gray-800 flex items-center justify-center">
      <PhoneIcon class="w-5 h-5 text-stone-500 dark:text-gray-400" />
    </div>
    <!-- Status dot: green = connected, silver = offline -->
    <span
      class="absolute -bottom-0.5 -right-0.5 w-3 h-3 rounded-full border-2 border-white dark:border-gray-900
             {device.is_connected ? 'bg-emerald-500' : 'bg-stone-300 dark:bg-gray-500'}"
      aria-label={device.is_connected ? $t('devices.status_connected') : lastSeenText()}
    ></span>
  </div>

  <!-- Device info -->
  <div class="flex-1 min-w-0">
    <p class="text-sm font-medium text-stone-800 dark:text-gray-100 truncate">
      {device.device_name}
    </p>
    <div class="flex items-center gap-1.5">
      <span class="text-xs text-stone-500 dark:text-gray-400 truncate">
        {device.device_model}
      </span>
      <span class="text-stone-300 dark:text-gray-600">&middot;</span>
      <span class="text-xs {device.is_connected ? 'text-emerald-600 dark:text-emerald-400 font-medium' : 'text-stone-500 dark:text-gray-400'}">
        {lastSeenText()}
      </span>
    </div>
    <p class="text-[11px] text-stone-400 dark:text-gray-500">
      {$t('devices.paired_date', { values: { date: formatDate(device.paired_at) } })}
    </p>
  </div>

  <!-- Inactive warning badge -->
  {#if isInactive}
    <div class="flex items-center gap-1 px-2 py-1 rounded-full bg-amber-50 dark:bg-amber-900/30 flex-shrink-0" title={$t('devices.inactive_warning', { values: { days: device.days_inactive } })}>
      <WarningIcon class="w-3.5 h-3.5 text-amber-500" />
      <span class="text-[10px] font-medium text-amber-600 dark:text-amber-400">{device.days_inactive}d</span>
    </div>
  {/if}

  <!-- Unpair button (hover-visible) -->
  <button
    class="flex-shrink-0 min-h-[36px] min-w-[36px] flex items-center justify-center
           rounded-lg text-stone-400 dark:text-gray-500
           hover:text-red-500 dark:hover:text-red-400
           hover:bg-red-50 dark:hover:bg-red-900/20
           opacity-0 group-hover:opacity-100 focus:opacity-100
           transition-all"
    onclick={() => onUnpair(device.device_id)}
    aria-label={$t('devices.unpair_button')}
  >
    <DeleteIcon class="w-4.5 h-4.5" />
  </button>
</div>
