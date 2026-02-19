<!-- ME-02: Unpair device confirmation dialog -->
<script lang="ts">
  import { tick } from 'svelte';
  import { t } from 'svelte-i18n';
  import { trapFocus, autoFocusFirst } from '$lib/utils/focus-trap';
  import Button from '$lib/components/ui/Button.svelte';

  let {
    deviceName,
    onConfirm,
    onCancel,
  }: {
    deviceName: string;
    onConfirm: () => void;
    onCancel: () => void;
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
     aria-label={$t('devices.unpair_title', { values: { name: deviceName } })}
     tabindex="-1"
     bind:this={dialogEl}
     onkeydown={(e) => { if (e.key === 'Escape') onCancel(); if (dialogEl) trapFocus(e, dialogEl); }}>
  <div class="dialog">
    <h3>{$t('devices.unpair_title', { values: { name: deviceName } })}</h3>
    <div class="body">
      <p>{$t('devices.unpair_consequences')}</p>
      <ul>
        <li>{$t('devices.unpair_disconnect')}</li>
        <li>{$t('devices.unpair_delete_data')}</li>
        <li>{$t('devices.unpair_revoke')}</li>
      </ul>
      <p>{$t('devices.unpair_repaired_note')}</p>
    </div>
    <div class="actions">
      <Button variant="secondary" onclick={onCancel}>{$t('common.cancel')}</Button>
      <Button variant="danger" onclick={onConfirm}>{$t('devices.unpair_confirm')}</Button>
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
</style>
