<!-- L4-01: Two-level category → subcategory selector with tappable cards. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { CATEGORIES, SUBCATEGORIES } from '$lib/types/journal';
  import Button from '$lib/components/ui/Button.svelte';
  import BackButton from '$lib/components/ui/BackButton.svelte';

  interface Props {
    onSelect: (category: string, specific: string) => void;
  }
  let { onSelect }: Props = $props();

  let selectedCategory: string | null = $state(null);
  let customText = $state('');

  const categoryKeys: Record<string, string> = {
    Pain: 'journal.category_pain',
    Digestive: 'journal.category_digestive',
    Respiratory: 'journal.category_respiratory',
    Neurological: 'journal.category_neurological',
    General: 'journal.category_general',
    Mood: 'journal.category_mood',
    Skin: 'journal.category_skin',
    Other: 'journal.category_other',
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
          {categoryKeys[cat] ? $t(categoryKeys[cat]) : cat}
        </span>
      </button>
    {/each}
  </div>
{:else}
  <!-- Subcategory list -->
  <BackButton
    label={selectedCategory ?? ''}
    onclick={() => { selectedCategory = null; customText = ''; }}
  />

  <div class="flex flex-col gap-2">
    {#each SUBCATEGORIES[selectedCategory] ?? [] as sub}
      {#if sub === 'Other'}
        <div class="flex gap-2">
          <input
            type="text"
            class="flex-1 px-4 py-3 rounded-xl border border-stone-200
                   text-stone-700 focus:outline-none focus:border-[var(--color-primary)]"
            placeholder={$t('journal.category_describe_placeholder')}
            maxlength={200}
            bind:value={customText}
          />
          <Button
            variant="primary"
            disabled={customText.trim().length === 0}
            onclick={() => onSelect(selectedCategory!, customText.trim())}
          >
            {$t('journal.category_next')}
          </Button>
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
