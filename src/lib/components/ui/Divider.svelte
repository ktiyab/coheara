<!--
  C13: Divider — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C13

  Section separator with optional centered label.
-->
<script lang="ts">
  interface Props {
    spacing?: 'sm' | 'md' | 'lg';
    label?: string;
  }

  let {
    spacing = 'md',
    label,
  }: Props = $props();

  const spacingClasses: Record<string, string> = {
    sm: 'my-2',
    md: 'my-4',
    lg: 'my-6',
  };

  let marginClass = $derived(spacingClasses[spacing]);
</script>

{#if label}
  <div class="flex items-center gap-3 {marginClass}">
    <div class="flex-1 border-t border-stone-200 dark:border-gray-700"></div>
    <span class="text-xs text-stone-400 dark:text-gray-500 font-medium uppercase tracking-wide">{label}</span>
    <div class="flex-1 border-t border-stone-200 dark:border-gray-700"></div>
  </div>
{:else}
  <hr class="border-stone-200 dark:border-gray-700 {marginClass}" />
{/if}
