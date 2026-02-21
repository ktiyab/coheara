<!--
  C11: Modal — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C11

  Overlay dialog for destructive confirmations ONLY (CP10).
  Focus trap, Escape close, backdrop close, scroll lock.
  Uses existing focus-trap.ts utility.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import type { Snippet } from 'svelte';
  import { t } from 'svelte-i18n';
  import { trapFocus, autoFocusFirst } from '$lib/utils/focus-trap';
  import { CloseIcon } from '$lib/components/icons/md';

  interface Props {
    open: boolean;
    title: string;
    onclose: () => void;
    children: Snippet;
    actions?: Snippet;
  }

  let {
    open,
    title,
    onclose,
    children,
    actions,
  }: Props = $props();

  let dialogEl: HTMLDivElement | undefined = $state(undefined);
  let previousFocus: HTMLElement | null = null;

  // Focus management on open/close
  $effect(() => {
    if (open && dialogEl) {
      previousFocus = document.activeElement as HTMLElement;
      document.body.style.overflow = 'hidden';
      tick().then(() => {
        if (dialogEl) autoFocusFirst(dialogEl);
      });
    }
    return () => {
      document.body.style.overflow = '';
      previousFocus?.focus();
    };
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onclose();
      return;
    }
    if (dialogEl) {
      trapFocus(e, dialogEl);
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onclose();
    }
  }

  let titleId = $derived(`modal-title-${title.replace(/\s+/g, '-').toLowerCase()}`);
</script>

{#if open}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 bg-black/40 z-30 flex items-center justify-center"
    onclick={handleBackdropClick}
    onkeydown={handleKeydown}
  >
    <div
      bind:this={dialogEl}
      class="bg-white dark:bg-gray-900 rounded-xl shadow-xl max-w-sm w-full mx-4 p-6"
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      tabindex="-1"
    >
      <!-- Header -->
      <div class="flex items-center justify-between mb-4">
        <h2 id={titleId} class="text-lg font-bold text-stone-800 dark:text-gray-100">{title}</h2>
        <button
          class="min-h-[44px] min-w-[44px] flex items-center justify-center
                 text-stone-400 dark:text-gray-500 hover:text-stone-600 dark:hover:text-gray-300 transition-colors"
          onclick={onclose}
          aria-label={$t('common.close')}
        >
          <CloseIcon class="w-5 h-5" />
        </button>
      </div>

      <!-- Content -->
      <div class="mb-6">
        {@render children()}
      </div>

      <!-- Actions -->
      {#if actions}
        <div class="flex gap-3 justify-end">
          {@render actions()}
        </div>
      {/if}
    </div>
  </div>
{/if}
