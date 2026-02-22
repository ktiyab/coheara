<!-- UA02-12: Unpair confirmation dialog — Tailwind + dark mode -->
<script lang="ts">
  import { tick } from 'svelte';
  import { t } from 'svelte-i18n';
  import { trapFocus, autoFocusFirst } from '$lib/utils/focus-trap';
  import { WarningIcon } from '$lib/components/icons/md';
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
<div
  class="fixed inset-0 z-[100] flex items-center justify-center bg-black/50"
  role="dialog"
  aria-modal="true"
  aria-label={$t('devices.unpair_title', { values: { name: deviceName } })}
  tabindex="-1"
  bind:this={dialogEl}
  onkeydown={(e) => { if (e.key === 'Escape') onCancel(); if (dialogEl) trapFocus(e, dialogEl); }}
>
  <div class="bg-white dark:bg-gray-900 rounded-xl p-6 max-w-sm w-[90%] shadow-xl border border-stone-200 dark:border-gray-700">
    <!-- Header with warning icon -->
    <div class="flex items-center gap-3 mb-4">
      <div class="w-10 h-10 rounded-full bg-red-50 dark:bg-red-900/30 flex items-center justify-center flex-shrink-0">
        <WarningIcon class="w-5 h-5 text-red-500" />
      </div>
      <h3 class="text-base font-semibold text-stone-800 dark:text-gray-100">
        {$t('devices.unpair_title', { values: { name: deviceName } })}
      </h3>
    </div>

    <!-- Consequences -->
    <div class="text-sm text-stone-600 dark:text-gray-300 space-y-2 mb-4">
      <p>{$t('devices.unpair_consequences')}</p>
      <ul class="list-disc pl-5 space-y-1 text-stone-500 dark:text-gray-400">
        <li>{$t('devices.unpair_disconnect')}</li>
        <li>{$t('devices.unpair_delete_data')}</li>
        <li>{$t('devices.unpair_revoke')}</li>
      </ul>
      <p class="text-xs text-stone-400 dark:text-gray-500">{$t('devices.unpair_repaired_note')}</p>
    </div>

    <!-- Actions -->
    <div class="flex justify-end gap-2">
      <Button variant="secondary" onclick={onCancel}>{$t('common.cancel')}</Button>
      <Button variant="danger" onclick={onConfirm}>{$t('devices.unpair_confirm')}</Button>
    </div>
  </div>
</div>
