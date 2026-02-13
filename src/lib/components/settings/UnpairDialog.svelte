<!-- ME-02: Unpair device confirmation dialog -->
<script lang="ts">
  let {
    deviceName,
    onConfirm,
    onCancel,
  }: {
    deviceName: string;
    onConfirm: () => void;
    onCancel: () => void;
  } = $props();
</script>

<div class="overlay" role="dialog" aria-modal="true">
  <div class="dialog">
    <h3>Unpair "{deviceName}"?</h3>
    <div class="body">
      <p>This will:</p>
      <ul>
        <li>Disconnect the device immediately</li>
        <li>Delete all cached health data from the phone</li>
        <li>Revoke the phone's access to your data</li>
      </ul>
      <p>The phone will need to be re-paired to connect again.</p>
    </div>
    <div class="actions">
      <button class="cancel-btn" onclick={onCancel}>Cancel</button>
      <button class="confirm-btn" onclick={onConfirm}>Unpair</button>
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
    border-radius: 0.5rem;
    padding: 1.5rem;
    max-width: 24rem;
    width: 90%;
  }
  .dialog h3 {
    margin: 0 0 1rem;
    font-size: 1.05rem;
  }
  .body {
    font-size: 0.9rem;
    color: var(--text-secondary, #475569);
    line-height: 1.5;
  }
  .body p {
    margin: 0 0 0.5rem;
  }
  .body ul {
    margin: 0 0 0.75rem;
    padding-left: 1.25rem;
  }
  .body li {
    margin-bottom: 0.25rem;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 1rem;
  }
  .cancel-btn {
    padding: 0.5rem 1rem;
    border: 1px solid var(--border-color, #e2e8f0);
    background: transparent;
    border-radius: 0.25rem;
    cursor: pointer;
  }
  .confirm-btn {
    padding: 0.5rem 1rem;
    background: var(--danger-color, #ef4444);
    color: white;
    border: none;
    border-radius: 0.25rem;
    cursor: pointer;
  }
  .confirm-btn:hover {
    opacity: 0.9;
  }
</style>
