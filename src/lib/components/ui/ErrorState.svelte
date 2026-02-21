<!--
  C4: ErrorState — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C4
  Replaces: 10+ inline error displays

  Centered error message with optional retry button.
  Composes Button (C1) for the retry action.
-->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import Button from './Button.svelte';
  import { ExclamationCircleSolid } from 'flowbite-svelte-icons';

  interface Props {
    message: string;
    onretry?: () => void;
    retryLabel?: string;
    severity?: 'error' | 'warning';
  }

  let {
    message,
    onretry,
    retryLabel,
    severity = 'error',
  }: Props = $props();

  let textColor = $derived(severity === 'error' ? 'text-red-600 dark:text-red-400' : 'text-amber-600 dark:text-amber-400');
  let iconColor = $derived(severity === 'error' ? 'text-red-500 dark:text-red-400' : 'text-amber-500 dark:text-amber-400');
</script>

<div
  class="flex flex-col items-center justify-center px-6 py-8 text-center"
  role="alert"
>
  <div class="flex items-center gap-2 mb-4">
    <ExclamationCircleSolid class="w-5 h-5 flex-shrink-0 {iconColor}" />
    <p class={textColor}>{message}</p>
  </div>
  {#if onretry}
    <Button variant="secondary" onclick={onretry}>
      {retryLabel ?? $t('common.retry')}
    </Button>
  {/if}
</div>
