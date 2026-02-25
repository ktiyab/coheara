<!--
  C1: Button — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C1
  Replaces: 20+ inline button implementations

  5 variants: primary, secondary, ghost, danger, dashed
  4 sizes: xs, sm, md, lg
  States: default, hover, active, disabled, loading, focus
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import { Spinner } from 'flowbite-svelte';

  interface Props {
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger' | 'dashed';
    size?: 'xs' | 'sm' | 'md' | 'lg';
    disabled?: boolean;
    loading?: boolean;
    fullWidth?: boolean;
    type?: 'button' | 'submit' | 'reset';
    ariaLabel?: string;
    onclick?: (e: MouseEvent) => void;
    children: Snippet;
  }

  let {
    variant = 'primary',
    size = 'md',
    disabled = false,
    loading = false,
    fullWidth = false,
    type = 'button',
    ariaLabel,
    onclick,
    children,
  }: Props = $props();

  const variantClasses: Record<string, string> = {
    primary: 'bg-[var(--color-success)] text-white hover:bg-[var(--color-success-800)] active:bg-[var(--color-success-800)]',
    secondary: 'bg-[var(--color-success-50)] text-[var(--color-success-800)] hover:bg-[var(--color-success-200)] active:bg-[var(--color-success-200)]',
    ghost: 'bg-transparent text-[var(--color-success)] border border-[var(--color-success)] hover:bg-[var(--color-success-50)] active:bg-[var(--color-success-200)]',
    danger: 'bg-red-600 text-white hover:bg-red-700 active:bg-red-800',
    dashed: 'bg-transparent text-stone-500 dark:text-gray-400 border border-dashed border-stone-300 dark:border-gray-600 hover:bg-stone-50 dark:hover:bg-gray-800 hover:border-stone-400 dark:hover:border-gray-500',
  };

  const sizeClasses: Record<string, string> = {
    xs: 'px-4 py-0 min-h-[36px] text-[13px] font-semibold',
    sm: 'px-3 py-1.5 min-h-[36px] text-sm',
    md: 'px-4 py-2.5 min-h-[44px] text-sm font-medium',
    lg: 'px-6 py-3 min-h-[48px] text-base font-medium',
  };

  let classes = $derived(
    `inline-flex items-center justify-center gap-2 rounded-lg transition-colors
     focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-primary)]
     ${sizeClasses[size]}
     ${variantClasses[variant]}
     ${fullWidth ? 'w-full' : ''}
     ${disabled || loading ? 'opacity-50 cursor-not-allowed pointer-events-none' : 'cursor-pointer active:scale-[0.98]'}`
  );
</script>

<button
  {type}
  class={classes}
  {onclick}
  disabled={disabled || loading}
  aria-label={ariaLabel}
  aria-disabled={disabled || loading}
  aria-busy={loading}
>
  {#if loading}
    <Spinner size="4" color="gray" />
    <span class="sr-only">{@render children()}</span>
  {:else}
    {@render children()}
  {/if}
</button>
