<!-- L4-01: Two-level category → subcategory selector with tappable cards. -->
<script lang="ts">
  import { CATEGORIES, SUBCATEGORIES } from '$lib/types/journal';

  interface Props {
    onSelect: (category: string, specific: string) => void;
  }
  let { onSelect }: Props = $props();

  let selectedCategory: string | null = $state(null);
  let customText = $state('');

  const categoryLabels: Record<string, string> = {
    Pain: 'Pain',
    Digestive: 'Stomach',
    Respiratory: 'Breathing',
    Neurological: 'Neuro',
    General: 'General',
    Mood: 'Mood',
    Skin: 'Skin',
    Other: 'Other',
  };
</script>

{#if !selectedCategory}
  <!-- Category grid (2x4) -->
  <div class="grid grid-cols-4 gap-3">
    {#each CATEGORIES as cat}
      <button
        class="flex flex-col items-center justify-center gap-2 p-4 rounded-xl
               bg-white border border-stone-200 hover:border-[var(--color-primary)]
               hover:bg-stone-50 transition-colors min-h-[80px]"
        onclick={() => { selectedCategory = cat; }}
      >
        <span class="text-xs text-stone-600 font-medium text-center">
          {categoryLabels[cat] ?? cat}
        </span>
      </button>
    {/each}
  </div>
{:else}
  <!-- Subcategory list -->
  <button
    class="text-sm text-stone-500 mb-3 min-h-[44px]"
    onclick={() => { selectedCategory = null; customText = ''; }}
  >
    &larr; {selectedCategory}
  </button>

  <div class="flex flex-col gap-2">
    {#each SUBCATEGORIES[selectedCategory] ?? [] as sub}
      {#if sub === 'Other'}
        <div class="flex gap-2">
          <input
            type="text"
            class="flex-1 px-4 py-3 rounded-xl border border-stone-200
                   text-stone-700 focus:outline-none focus:border-[var(--color-primary)]"
            placeholder="Describe..."
            maxlength={200}
            bind:value={customText}
          />
          <button
            class="px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
                   font-medium min-h-[44px] disabled:opacity-50"
            disabled={customText.trim().length === 0}
            onclick={() => onSelect(selectedCategory!, customText.trim())}
          >
            Next
          </button>
        </div>
      {:else}
        <button
          class="w-full text-left px-4 py-3 rounded-xl bg-white border border-stone-200
                 hover:border-[var(--color-primary)] hover:bg-stone-50
                 text-stone-700 transition-colors min-h-[44px]"
          onclick={() => onSelect(selectedCategory!, sub)}
        >
          {sub}
        </button>
      {/if}
    {/each}
  </div>
{/if}
