<!-- M0-02: Desktop confirmation dialog for incoming pairing request -->
<script lang="ts">
  import { tick } from 'svelte';
  import { t } from 'svelte-i18n';
  import type { PendingApproval } from '$lib/types/pairing';
  import { trapFocus, autoFocusFirst } from '$lib/utils/focus-trap';
  import Button from '$lib/components/ui/Button.svelte';

  let {
    pending,
    onApprove,
    onDeny,
  }: {
    pending: PendingApproval;
    onApprove: () => void;
    onDeny: () => void;
  } = $props();

  let dialogEl: HTMLDivElement | undefined = $state(undefined);

  $effect(() => {
    if (dialogEl) {
      tick().then(() => { if (dialogEl) autoFocusFirst(dialogEl); });
    }
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="overlay" role="dialog" aria-modal="true"
     aria-label={$t('pairing.new_device_title')}
     tabindex="-1"
     bind:this={dialogEl}
     onkeydown={(e) => { if (e.key === 'Escape') onDeny(); if (dialogEl) trapFocus(e, dialogEl); }}>
  <div class="dialog">
    <h3>{$t('pairing.new_device_title')}</h3>
    <div class="device-info">
      <p class="device-model">{pending.device_model}</p>
      <p class="device-name">"{pending.device_name}"</p>
    </div>
    <p class="body">{$t('pairing.allow_access')}</p>
    <div class="actions">
      <Button variant="secondary" onclick={onDeny}>{$t('pairing.deny')}</Button>
      <Button variant="primary" onclick={onApprove}>{$t('pairing.allow')}</Button>
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
</style>
