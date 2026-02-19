<!--
  C5: EmptyState — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C5
  Replaces: 3+ varying empty state implementations

  Centered empty content pattern with icon, title, description, and optional CTA.
  Composes Button (C1) for the action.
-->
<script lang="ts">
  import Button from './Button.svelte';

  interface Props {
    icon: string;
    title: string;
    description?: string;
    actionLabel?: string;
    onaction?: () => void;
  }

  let {
    icon,
    title,
    description,
    actionLabel,
    onaction,
  }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center px-8 py-12 text-center">
  <div class="w-24 h-24 bg-stone-100 rounded-2xl flex items-center justify-center mb-6">
    <span class="text-4xl text-stone-300" aria-hidden="true">{icon}</span>
  </div>
  <p class="text-lg font-medium text-stone-700 mb-2">{title}</p>
  {#if description}
    <p class="text-sm text-stone-500 mb-6 max-w-[280px]">{description}</p>
  {/if}
  {#if actionLabel && onaction}
    <Button variant="primary" size="lg" onclick={onaction}>
      {actionLabel}
    </Button>
  {/if}
</div>
