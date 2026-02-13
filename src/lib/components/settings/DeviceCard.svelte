<!-- ME-02: Single paired device row -->
<script lang="ts">
  import type { DeviceSummary } from '$lib/types/devices';

  let { device, onUnpair }: { device: DeviceSummary; onUnpair: (id: string) => void } = $props();

  function formatDate(iso: string): string {
    return new Date(iso).toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  }

  function lastSeenText(): string {
    if (device.is_connected) return 'Connected';
    if (!device.days_inactive || device.days_inactive === 0) return 'Last seen: just now';
    if (device.days_inactive === 1) return 'Last seen: yesterday';
    return `Last seen: ${device.days_inactive} days ago`;
  }

  const isInactive = $derived((device.days_inactive ?? 0) >= 30);
</script>

<div class="device-card" class:inactive={isInactive}>
  <div class="device-header">
    <span class="device-icon">
      {#if device.device_model.toLowerCase().includes('iphone')}
        iPhone
      {:else}
        Android
      {/if}
    </span>
    <div class="device-info">
      <h4>{device.device_name}</h4>
      <p class="device-status">
        <span class="status-dot" class:connected={device.is_connected}></span>
        {lastSeenText()}
      </p>
      <p class="paired-date">Paired: {formatDate(device.paired_at)}</p>
    </div>
  </div>

  {#if isInactive}
    <div class="inactive-warning">
      Inactive for {device.days_inactive} days. Consider unpairing for security.
    </div>
  {/if}

  <button class="unpair-btn" onclick={() => onUnpair(device.device_id)}>
    Unpair Device
  </button>
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
  .unpair-btn {
    margin-top: 0.5rem;
    padding: 0.375rem 0.75rem;
    font-size: 0.8rem;
    border: 1px solid var(--danger-color, #ef4444);
    color: var(--danger-color, #ef4444);
    background: transparent;
    border-radius: 0.25rem;
    cursor: pointer;
  }
  .unpair-btn:hover {
    background: var(--danger-bg, #fef2f2);
  }
</style>
