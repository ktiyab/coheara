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

  let textColor = $derived(severity === 'error' ? 'text-red-600' : 'text-amber-600');
</script>

<div
  class="flex flex-col items-center justify-center px-6 py-8 text-center"
  role="alert"
>
  <p class="{textColor} mb-4">{message}</p>
  {#if onretry}
    <Button variant="secondary" onclick={onretry}>
      {retryLabel ?? $t('common.retry')}
    </Button>
  {/if}
</div>
