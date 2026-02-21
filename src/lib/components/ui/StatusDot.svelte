<!-- AUDIT_01 Section 6A: Reusable 8px status dot + label. -->
<script lang="ts">
  type Variant = 'success' | 'info' | 'warning' | 'neutral';

  let { label, variant = 'success' as Variant, class: className = '' }:
    { label: string; variant?: Variant; class?: string } = $props();

  const colors: Record<Variant, string> = {
    success: 'bg-[var(--color-success)]',
    info: 'bg-[var(--color-interactive)]',
    warning: 'bg-[var(--color-warning)]',
    neutral: 'bg-stone-300 dark:bg-gray-600',
  };
</script>

<span class="inline-flex items-center gap-2 {className}">
  <span class="w-2 h-2 rounded-full {colors[variant]} flex-shrink-0"></span>
  <span class="text-[var(--text-caption)] text-stone-500 dark:text-gray-400">{label}</span>
</span>
