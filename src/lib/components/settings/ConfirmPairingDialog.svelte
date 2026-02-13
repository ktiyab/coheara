<!-- M0-02: Desktop confirmation dialog for incoming pairing request -->
<script lang="ts">
  import type { PendingApproval } from '$lib/types/pairing';

  let {
    pending,
    onApprove,
    onDeny,
  }: {
    pending: PendingApproval;
    onApprove: () => void;
    onDeny: () => void;
  } = $props();
</script>

<div class="overlay" role="dialog" aria-modal="true">
  <div class="dialog">
    <h3>New Device Wants to Connect</h3>
    <div class="device-info">
      <p class="device-model">{pending.device_model}</p>
      <p class="device-name">"{pending.device_name}"</p>
    </div>
    <p class="body">Allow this device to access your health data?</p>
    <div class="actions">
      <button class="deny-btn" onclick={onDeny}>Deny</button>
      <button class="allow-btn" onclick={onApprove}>Allow</button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .dialog {
    background: var(--bg-primary, white);
    border-radius: 0.75rem;
    padding: 1.5rem;
    max-width: 22rem;
    width: 90%;
    text-align: center;
  }
  .dialog h3 {
    margin: 0 0 1rem;
    font-size: 1.05rem;
  }
  .device-info {
    margin-bottom: 1rem;
  }
  .device-model {
    margin: 0;
    font-size: 0.9rem;
    color: var(--text-secondary, #64748b);
  }
  .device-name {
    margin: 0.25rem 0 0;
    font-size: 1rem;
    font-weight: 600;
  }
  .body {
    font-size: 0.9rem;
    color: var(--text-secondary, #475569);
    margin: 0 0 1rem;
  }
  .actions {
    display: flex;
    justify-content: center;
    gap: 0.75rem;
  }
  .deny-btn {
    padding: 0.5rem 1.25rem;
    border: 1px solid var(--border-color, #e2e8f0);
    background: transparent;
    border-radius: 0.375rem;
    cursor: pointer;
    font-size: 0.9rem;
  }
  .allow-btn {
    padding: 0.5rem 1.25rem;
    background: var(--accent-color, #0d9488);
    color: white;
    border: none;
    border-radius: 0.375rem;
    cursor: pointer;
    font-size: 0.9rem;
  }
  .allow-btn:hover {
    opacity: 0.9;
  }
</style>
