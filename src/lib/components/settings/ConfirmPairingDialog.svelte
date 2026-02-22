<!-- UA02-12: Pairing approval dialog — Tailwind + dark mode -->
<script lang="ts">
  import { tick } from 'svelte';
  import { t } from 'svelte-i18n';
  import type { PendingApproval } from '$lib/types/pairing';
  import { trapFocus, autoFocusFirst } from '$lib/utils/focus-trap';
  import { PhoneIcon } from '$lib/components/icons/md';
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
<div
  class="fixed inset-0 z-[100] flex items-center justify-center bg-black/50"
  role="dialog"
  aria-modal="true"
  aria-label={$t('pairing.new_device_title')}
  tabindex="-1"
  bind:this={dialogEl}
  onkeydown={(e) => { if (e.key === 'Escape') onDeny(); if (dialogEl) trapFocus(e, dialogEl); }}
>
  <div class="bg-white dark:bg-gray-900 rounded-xl p-6 max-w-[22rem] w-[90%] shadow-xl border border-stone-200 dark:border-gray-700 text-center">
    <!-- Icon -->
    <div class="w-14 h-14 mx-auto mb-4 rounded-full bg-teal-50 dark:bg-teal-900/30 flex items-center justify-center">
      <PhoneIcon class="w-7 h-7 text-[var(--color-interactive)]" />
    </div>

    <h3 class="text-base font-semibold text-stone-800 dark:text-gray-100 mb-3">
      {$t('pairing.new_device_title')}
    </h3>

    <!-- Device info -->
    <div class="mb-4 py-3 px-4 bg-stone-50 dark:bg-gray-800 rounded-lg">
      <p class="text-sm text-stone-500 dark:text-gray-400">{pending.device_model}</p>
      <p class="text-base font-semibold text-stone-800 dark:text-gray-100 mt-0.5">"{pending.device_name}"</p>
    </div>

    <p class="text-sm text-stone-500 dark:text-gray-400 mb-5">
      {$t('pairing.allow_access')}
    </p>

    <!-- Actions -->
    <div class="flex justify-center gap-3">
      <Button variant="secondary" onclick={onDeny}>{$t('pairing.deny')}</Button>
      <Button variant="primary" onclick={onApprove}>{$t('pairing.allow')}</Button>
    </div>
  </div>
</div>
