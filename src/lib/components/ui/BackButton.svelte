<!--
  C6: BackButton — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C6
  Replaces: 12 identical back navigation buttons

  Defaults to navigation.goBack() — zero-config for 90% of cases.
-->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { ChevronLeftIcon } from '$lib/components/icons/md';

  interface Props {
    label?: string;
    ariaLabel?: string;
    onclick?: () => void;
  }

  let {
    label,
    ariaLabel,
    onclick,
  }: Props = $props();
</script>

<button
  class="min-h-[44px] min-w-[44px] flex items-center gap-1
         text-stone-500 dark:text-gray-400 hover:text-stone-700 dark:hover:text-gray-200 transition-colors
         focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-primary)]"
  onclick={onclick ?? (() => navigation.goBack())}
  aria-label={ariaLabel ?? $t('common.go_back')}
>
  <ChevronLeftIcon class="w-4 h-4" />
  {#if label}
    <span class="text-sm">{label}</span>
  {/if}
</button>
