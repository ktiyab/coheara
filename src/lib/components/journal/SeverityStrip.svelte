<!-- V14: Flat numbered 1-5 severity strip replacing SVG face-based SeverityScale. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { SEVERITY_COLORS, SEVERITY_LABELS } from '$lib/types/journal';

  interface Props {
    value: number;
    onChange: (value: number) => void;
  }
  let { value, onChange }: Props = $props();

  const levels = [1, 2, 3, 4, 5];

  function textColor(level: number): string {
    return level >= 4 ? 'white' : '#1c1917';
  }
</script>

<div role="radiogroup" aria-label={$t('journal.entry_severity_label')} class="flex flex-col gap-2">
  <div class="flex items-center gap-2">
    {#each levels as level}
      {@const selected = value === level}
      <button
        role="radio"
        aria-checked={selected}
        aria-label="{level} - {SEVERITY_LABELS[level]}"
        class="flex-1 min-h-[44px] rounded-lg text-sm font-medium transition-all border"
        style={selected
          ? `background-color: ${SEVERITY_COLORS[level]}; color: ${textColor(level)}; border-color: ${SEVERITY_COLORS[level]};`
          : 'background-color: transparent; border-color: #d6d3d1;'}
        class:dark:border-gray-600={!selected}
        onclick={() => onChange(level)}
      >
        {level}
      </button>
    {/each}
  </div>
  <div class="flex justify-between text-xs text-stone-500 dark:text-gray-400 px-1">
    <span>{$t('journal.entry_mild')}</span>
    <span>{$t('journal.entry_severe')}</span>
  </div>
</div>
