<!--
  C12: Avatar — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C12

  User/AI avatar with initial letter.
  3 sizes, 2 variants (user=teal, ai=stone).
-->
<script lang="ts">
  interface Props {
    name: string;
    size?: 'sm' | 'md' | 'lg';
    variant?: 'user' | 'ai';
    /** Optional override color (Spec 45 [PU-04]: profile-specific). */
    color?: string | null;
  }

  let {
    name,
    size = 'md',
    variant = 'user',
    color = null,
  }: Props = $props();

  let initial = $derived(name.charAt(0).toUpperCase());

  const sizeClasses: Record<string, string> = {
    sm: 'w-6 h-6 text-xs',
    md: 'w-8 h-8 text-sm',
    lg: 'w-10 h-10 text-base',
  };

  const variantClasses: Record<string, string> = {
    user: 'bg-teal-600 text-white',
    ai: 'bg-stone-200 text-stone-600',
  };

  let classes = $derived(
    `inline-flex items-center justify-center rounded-full font-medium flex-shrink-0
     ${sizeClasses[size]} ${variantClasses[variant]}`
  );
</script>

{#if color}
  <span class={classes} aria-hidden="true"
    style:background-color={color}
    style:color="white"
  >
    {initial}
  </span>
{:else}
  <span class={classes} aria-hidden="true">
    {initial}
  </span>
{/if}
