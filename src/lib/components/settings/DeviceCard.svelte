<!-- ME-02: Single paired device row -->
<script lang="ts">
  import { t, locale } from 'svelte-i18n';
  import type { DeviceSummary } from '$lib/types/devices';
  import Button from '$lib/components/ui/Button.svelte';

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

<div class="device-card" class:inactive={isInactive}>
  <div class="device-header">
    <span class="device-icon">
      {#if device.device_model.toLowerCase().includes('iphone')}
        {$t('devices.type_iphone')}
      {:else}
        {$t('devices.type_android')}
      {/if}
    </span>
    <div class="device-info">
      <h4>{device.device_name}</h4>
      <p class="device-status">
        <span class="status-dot" class:connected={device.is_connected}></span>
        {lastSeenText()}
      </p>
      <p class="paired-date">{$t('devices.paired_date', { values: { date: formatDate(device.paired_at) } })}</p>
    </div>
  </div>

  {#if isInactive}
    <div class="inactive-warning">
      {$t('devices.inactive_warning', { values: { days: device.days_inactive } })}
    </div>
  {/if}

  <Button variant="danger" size="sm" onclick={() => onUnpair(device.device_id)}>
    {$t('devices.unpair_button')}
  </Button>
</div>

<style>
  .device-card {
    border: 1px solid var(--border-color, #e2e8f0);
    border-radius: 0.5rem;
    padding: 1rem;
    margin-bottom: 0.75rem;
  }
  .device-card.inactive {
    border-color: var(--warning-color, #f59e0b);
  }
  .device-header {
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
  }
  .device-icon {
    font-size: 0.75rem;
    color: var(--text-secondary, #64748b);
    min-width: 3.5rem;
    text-align: center;
    padding-top: 0.25rem;
  }
  .device-info {
    flex: 1;
  }
  .device-info h4 {
    margin: 0 0 0.25rem;
    font-size: 0.95rem;
  }
  .device-status {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    margin: 0 0 0.125rem;
    font-size: 0.85rem;
    color: var(--text-secondary, #64748b);
  }
  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-secondary, #94a3b8);
  }
  .status-dot.connected {
    background: var(--success-color, #22c55e);
  }
  .paired-date {
    margin: 0;
    font-size: 0.8rem;
    color: var(--text-tertiary, #94a3b8);
  }
  .inactive-warning {
    margin: 0.5rem 0;
    padding: 0.5rem;
    background: var(--warning-bg, #fffbeb);
    color: var(--warning-color, #b45309);
    border-radius: 0.25rem;
    font-size: 0.8rem;
  }
</style>
