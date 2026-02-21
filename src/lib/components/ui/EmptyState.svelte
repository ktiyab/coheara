<!--
  C5: EmptyState — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C5
  XC-2: Flat icon (no container), teal CTA (--color-interactive)

  Centered empty content pattern with icon, title, description, and optional CTA.
-->
<script lang="ts">
  import type { Component } from 'svelte';

  interface Props {
    icon: Component<{class?: string}>;
    title: string;
    description?: string;
    actionLabel?: string;
    onaction?: () => void;
  }

  let {
    icon: Icon,
    title,
    description,
    actionLabel,
    onaction,
  }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center px-8 py-12 text-center">
  <Icon class="w-10 h-10 text-[var(--color-interactive)] mb-6" />
  <p class="text-lg font-medium text-stone-700 dark:text-gray-300 mb-2">{title}</p>
  {#if description}
    <p class="text-sm text-stone-500 dark:text-gray-400 mb-6 max-w-[280px]">{description}</p>
  {/if}
  {#if actionLabel && onaction}
    <button
      class="inline-flex items-center justify-center gap-2 rounded-lg px-6 py-3 min-h-[48px] text-base font-medium
             bg-[var(--color-interactive)] text-white
             hover:bg-[var(--color-interactive-hover)] active:bg-[var(--color-interactive-active)]
             focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-interactive)]
             cursor-pointer active:scale-[0.98] transition-colors"
      onclick={onaction}
    >
      {actionLabel}
    </button>
  {/if}
</div>
