<!-- L4-01: Face-based severity scale (1-5). SVG faces, no numbers shown to patient. -->
<script lang="ts">
  import { SEVERITY_LABELS, SEVERITY_COLORS } from '$lib/types/journal';

  interface Props {
    value: number;
    onChange: (value: number) => void;
    onNext: () => void;
  }
  let { value, onChange, onNext }: Props = $props();

  const levels = [1, 2, 3, 4, 5];
</script>

<div class="flex items-center justify-between gap-3 px-2 mb-6">
  {#each levels as level}
    <button
      class="flex flex-col items-center gap-2 transition-all min-h-[44px] min-w-[44px]"
      class:scale-110={value === level}
      aria-label={SEVERITY_LABELS[level]}
      onclick={() => onChange(level)}
    >
      <div
        class="w-14 h-14 rounded-full border-2 flex items-center justify-center transition-all"
        style="border-color: {value === level ? 'var(--color-primary)' : '#d6d3d1'};
               background-color: {value === level ? SEVERITY_COLORS[level] + '30' : 'transparent'}"
      >
        <svg viewBox="0 0 48 48" class="w-10 h-10">
          <circle cx="24" cy="24" r="22" fill={SEVERITY_COLORS[level]} opacity="0.3" />
          <circle cx="24" cy="24" r="22" fill="none" stroke={SEVERITY_COLORS[level]} stroke-width="2" />
          <!-- Eyes -->
          <circle cx="17" cy="20" r="2" fill={SEVERITY_COLORS[level]} />
          <circle cx="31" cy="20" r="2" fill={SEVERITY_COLORS[level]} />
          <!-- Mouth varies by severity -->
          {#if level === 1}
            <path d="M16 30 Q24 36 32 30" fill="none" stroke={SEVERITY_COLORS[level]} stroke-width="2" />
          {:else if level === 2}
            <path d="M16 31 Q24 34 32 31" fill="none" stroke={SEVERITY_COLORS[level]} stroke-width="2" />
          {:else if level === 3}
            <line x1="16" y1="32" x2="32" y2="32" stroke={SEVERITY_COLORS[level]} stroke-width="2" />
          {:else if level === 4}
            <path d="M16 34 Q24 30 32 34" fill="none" stroke={SEVERITY_COLORS[level]} stroke-width="2" />
          {:else}
            <path d="M16 36 Q24 28 32 36" fill="none" stroke={SEVERITY_COLORS[level]} stroke-width="2" />
          {/if}
        </svg>
      </div>
      <span class="text-xs text-stone-500 text-center leading-tight"
            class:font-medium={value === level}>
        {SEVERITY_LABELS[level]}
      </span>
    </button>
  {/each}
</div>

{#if value >= 1}
  <button
    class="w-full px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
           font-medium min-h-[44px]"
    onclick={onNext}
  >
    Next
  </button>
{/if}
