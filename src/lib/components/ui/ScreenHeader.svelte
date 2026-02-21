<!--
  C7: ScreenHeader — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C7
  Replaces: 12+ screen header implementations

  Composes BackButton (C6) and Button (C1).
  Three modes: action button, custom slot, or title-only.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import BackButton from './BackButton.svelte';
  import Button from './Button.svelte';

  interface Props {
    title: string;
    showBack?: boolean;
    onback?: () => void;
    actionLabel?: string;
    onaction?: () => void;
    children?: Snippet;
  }

  let {
    title,
    showBack = true,
    onback,
    actionLabel,
    onaction,
    children,
  }: Props = $props();
</script>

<header class="flex items-center gap-2 px-4 h-14">
  {#if showBack}
    <BackButton onclick={onback} />
  {/if}
  <h1 class="flex-1 text-xl font-bold text-stone-800 dark:text-gray-100 truncate">
    {title}
  </h1>
  {#if actionLabel && onaction}
    <Button variant="primary" size="sm" onclick={onaction}>
      {actionLabel}
    </Button>
  {:else if children}
    {@render children()}
  {/if}
</header>
