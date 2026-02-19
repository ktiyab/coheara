<!--
  C2: Badge — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C2
  Replaces: 8+ inline status badge implementations

  5 semantic variants: success, warning, danger, info, neutral
  2 sizes: sm, md
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

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

  const variantClasses: Record<string, string> = {
    success: 'bg-green-100 text-green-700',
    warning: 'bg-amber-100 text-amber-700',
    danger: 'bg-red-100 text-red-700',
    info: 'bg-blue-100 text-blue-700',
    neutral: 'bg-stone-100 text-stone-600',
  };

  const sizeClasses: Record<string, string> = {
    sm: 'px-2 py-0.5 text-xs',
    md: 'px-2.5 py-1 text-sm',
  };

  let classes = $derived(
    `inline-flex items-center rounded-full font-medium whitespace-nowrap flex-shrink-0
     ${variantClasses[variant]}
     ${sizeClasses[size]}`
  );
</script>

<span class={classes}>
  {@render children()}
</span>
