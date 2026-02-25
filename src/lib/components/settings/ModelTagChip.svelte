<!--
  CT-01: ModelTagChip — Reusable capability tag chip.

  Displays a colored chip for a CapabilityTag.
  Supports toggleable mode (click to add/remove) and static display.
-->
<script lang="ts">
  import type { CapabilityTag } from '$lib/types/ai';
  import { TAG_DISPLAY } from '$lib/types/ai';

  interface Props {
    tag: CapabilityTag;
    active?: boolean;
    toggleable?: boolean;
    ontoggle?: (tag: CapabilityTag) => void;
  }

  let {
    tag,
    active = true,
    toggleable = false,
    ontoggle,
  }: Props = $props();

  const info = $derived(TAG_DISPLAY[tag]);

  const colorClasses: Record<string, { active: string; inactive: string }> = {
    blue: {
      active: 'bg-blue-100 text-blue-800 dark:bg-blue-900/40 dark:text-blue-300 border-blue-200 dark:border-blue-700',
      inactive: 'bg-stone-100 text-stone-400 dark:bg-gray-800 dark:text-gray-500 border-stone-200 dark:border-gray-700',
    },
    green: {
      active: 'bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-300 border-emerald-200 dark:border-emerald-700',
      inactive: 'bg-stone-100 text-stone-400 dark:bg-gray-800 dark:text-gray-500 border-stone-200 dark:border-gray-700',
    },
    purple: {
      active: 'bg-purple-100 text-purple-800 dark:bg-purple-900/40 dark:text-purple-300 border-purple-200 dark:border-purple-700',
      inactive: 'bg-stone-100 text-stone-400 dark:bg-gray-800 dark:text-gray-500 border-stone-200 dark:border-gray-700',
    },
    amber: {
      active: 'bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-300 border-amber-200 dark:border-amber-700',
      inactive: 'bg-stone-100 text-stone-400 dark:bg-gray-800 dark:text-gray-500 border-stone-200 dark:border-gray-700',
    },
    gray: {
      active: 'bg-stone-200 text-stone-700 dark:bg-gray-700 dark:text-gray-300 border-stone-300 dark:border-gray-600',
      inactive: 'bg-stone-100 text-stone-400 dark:bg-gray-800 dark:text-gray-500 border-stone-200 dark:border-gray-700',
    },
  };

  const classes = $derived(
    colorClasses[info.color]?.[active ? 'active' : 'inactive'] ?? colorClasses.gray.active
  );
</script>

{#if toggleable}
  <button
    type="button"
    class="inline-flex items-center px-2 py-0.5 text-xs font-medium rounded border transition-colors {classes} {active ? '' : 'opacity-60'} hover:opacity-80 cursor-pointer"
    onclick={() => ontoggle?.(tag)}
    aria-pressed={active}
  >
    {info.label}
  </button>
{:else}
  <span
    class="inline-flex items-center px-2 py-0.5 text-xs font-medium rounded border {classes}"
  >
    {info.label}
  </span>
{/if}
