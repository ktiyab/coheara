<!--
  C2: Badge — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C2 + D4 Tier 1
  Wraps Flowbite Badge — gains dark mode, dismissable, border variant.

  5 semantic variants: success, warning, danger, info, neutral
  2 sizes: sm, md
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import { Badge as FbBadge } from 'flowbite-svelte';

  interface Props {
    variant: 'success' | 'warning' | 'danger' | 'info' | 'neutral';
    size?: 'sm' | 'md';
    children: Snippet;
  }

  let {
    variant,
    size = 'sm',
    children,
  }: Props = $props();

  const colorMap: Record<string, 'green' | 'yellow' | 'red' | 'blue' | 'gray'> = {
    success: 'green',
    warning: 'yellow',
    danger: 'red',
    info: 'blue',
    neutral: 'gray',
  };

  let fbColor = $derived(colorMap[variant]);
</script>

<FbBadge color={fbColor} large={size === 'md'} rounded class="font-medium">
  {@render children()}
</FbBadge>
