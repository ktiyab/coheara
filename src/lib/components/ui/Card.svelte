<!--
  C9: Card — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C9
  Replaces: 2+ inline card implementations

  When onclick is provided, renders as <button> for keyboard accessibility.
  White-on-stone visual foundation with warm shadow.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    onclick?: () => void;
    selected?: boolean;
    padding?: 'sm' | 'md' | 'lg';
    children: Snippet;
  }

  let {
    onclick,
    selected = false,
    padding = 'md',
    children,
  }: Props = $props();

  const paddingClasses: Record<string, string> = {
    sm: 'p-3',
    md: 'p-4',
    lg: 'p-6',
  };

  let baseClasses = $derived(
    `bg-white dark:bg-gray-900 rounded-xl shadow-sm border
     ${paddingClasses[padding]}
     ${selected
       ? 'ring-2 ring-[var(--color-primary)] ring-offset-1 border-transparent'
       : 'border-stone-100 dark:border-gray-700'}`
  );

  let clickableClasses = $derived(
    `${baseClasses} w-full text-left cursor-pointer
     hover:shadow-md dark:hover:shadow-gray-800/50 transition-shadow min-h-[44px]
     focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-primary)]`
  );
</script>

{#if onclick}
  <button class={clickableClasses} {onclick}>
    {@render children()}
  </button>
{:else}
  <div class={baseClasses}>
    {@render children()}
  </div>
{/if}
