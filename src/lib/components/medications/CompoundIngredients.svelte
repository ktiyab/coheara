<!-- L3-05: Compound ingredients list for compound medications. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { CompoundIngredientView } from '$lib/types/medication';

  interface Props {
    ingredients: CompoundIngredientView[];
  }
  let { ingredients }: Props = $props();
</script>

<div>
  <h3 class="text-sm font-medium text-stone-500 dark:text-gray-400 mb-3">{$t('medications.compound_heading')}</h3>
  <div class="flex flex-col gap-2">
    {#each ingredients as ingredient}
      <div class="flex items-baseline gap-2 px-3 py-2 bg-white dark:bg-gray-900 rounded-lg border border-stone-100 dark:border-gray-800">
        <span class="text-sm font-medium text-stone-800 dark:text-gray-100">
          {ingredient.ingredient_name}
        </span>
        {#if ingredient.ingredient_dose}
          <span class="text-sm text-stone-600 dark:text-gray-300">{ingredient.ingredient_dose}</span>
        {/if}
        {#if ingredient.maps_to_generic}
          <span class="text-xs text-stone-500 dark:text-gray-400">({$t('medications.compound_also_known', { values: { generic: ingredient.maps_to_generic } })})</span>
        {/if}
      </div>
    {/each}
  </div>
</div>
