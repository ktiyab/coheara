<!-- L4-04: Zoom level selector — floating panel with Day/Week/Month/Year. -->
<script lang="ts">
  import type { ZoomLevel } from '$lib/types/timeline';

  interface Props {
    currentZoom: ZoomLevel;
    onZoomChange: (zoom: ZoomLevel) => void;
  }
  let { currentZoom, onZoomChange }: Props = $props();

  const levels: ZoomLevel[] = ['Day', 'Week', 'Month', 'Year'];
</script>

<div class="absolute bottom-4 right-4 flex flex-col bg-white dark:bg-gray-900 rounded-xl shadow-lg
            border border-stone-200 dark:border-gray-700 overflow-hidden z-10"
     role="radiogroup"
     aria-label="Timeline zoom level">
  {#each levels as level}
    <button
      class="px-4 py-2 text-sm min-h-[44px] min-w-[44px] transition-colors
             {currentZoom === level
               ? 'bg-stone-800 dark:bg-gray-100 text-white dark:text-gray-900 font-medium'
               : 'text-stone-600 dark:text-gray-300 hover:bg-stone-50 dark:hover:bg-gray-800'}"
      role="radio"
      aria-checked={currentZoom === level}
      onclick={() => onZoomChange(level)}
    >
      {level}
    </button>
  {/each}
</div>
