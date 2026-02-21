<!-- V14: Symptom history list with filters, grouped by date. Uses SymptomCard. -->
<script lang="ts">
  import { t, locale } from 'svelte-i18n';
  import { resolveSymptom, deleteSymptom } from '$lib/api/journal';
  import type { StoredSymptom } from '$lib/types/journal';
  import { CATEGORIES } from '$lib/types/journal';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import SymptomCard from './SymptomCard.svelte';
  import { Checkbox } from 'flowbite-svelte';

  interface Props {
    symptoms: StoredSymptom[];
    loading: boolean;
    onRefresh: () => Promise<void>;
  }
  let { symptoms, loading, onRefresh }: Props = $props();

  let filterCategory = $state('all');
  let filterActive = $state(false);

  let filtered = $derived.by(() => {
    const syms: StoredSymptom[] = symptoms;
    return syms.filter(s => {
      if (filterCategory !== 'all' && s.category !== filterCategory) return false;
      if (filterActive && !s.still_active) return false;
      return true;
    });
  });

  let grouped = $derived.by(() => {
    const groups = new Map<string, StoredSymptom[]>();
    const items: StoredSymptom[] = filtered;
    for (const s of items) {
      const dateKey = new Date(s.recorded_date).toLocaleDateString($locale ?? 'en', {
        month: 'short', day: 'numeric', year: 'numeric',
      });
      if (!groups.has(dateKey)) groups.set(dateKey, []);
      groups.get(dateKey)!.push(s);
    }
    return groups;
  });

  async function handleResolve(id: string) {
    try {
      await resolveSymptom(id);
      await onRefresh();
    } catch (e) {
      console.error('Failed to resolve symptom:', e);
    }
  }

  async function handleDelete(id: string) {
    try {
      await deleteSymptom(id);
      await onRefresh();
    } catch (e) {
      console.error('Failed to delete symptom:', e);
    }
  }
</script>

<div class="px-6">
  <!-- Filters -->
  <div class="flex gap-3 mb-4 items-center">
    <select
      class="px-3 py-2 rounded-lg border border-stone-200 dark:border-gray-700 text-sm text-stone-700 dark:text-gray-200
             bg-white dark:bg-gray-900 min-h-[44px] focus:border-[var(--color-primary)] focus:outline-none"
      bind:value={filterCategory}
      aria-label={$t('journal.history_all_categories')}
    >
      <option value="all">{$t('journal.history_all_categories')}</option>
      {#each CATEGORIES as cat}
        <option value={cat}>{$t(`journal.category_${cat.toLowerCase()}`)}</option>
      {/each}
    </select>

    <Checkbox bind:checked={filterActive} color="primary" class="min-h-[44px]">
      <span class="text-sm text-stone-600 dark:text-gray-400">{$t('journal.history_active_only')}</span>
    </Checkbox>
  </div>

  {#if loading}
    <LoadingState variant="inline" />
  {:else if filtered.length === 0}
    <div class="text-center py-12">
      <p class="text-stone-500 dark:text-gray-400 mb-2">{$t('journal.history_empty_title')}</p>
      <p class="text-sm text-stone-500 dark:text-gray-400">{$t('journal.history_empty_hint')}</p>
    </div>
  {:else}
    {#each [...grouped.entries()] as [date, items]}
      <h3 class="text-xs font-medium text-stone-500 dark:text-gray-400 uppercase mt-4 mb-2">{date}</h3>
      {#each items as symptom (symptom.id)}
        <div class="mb-2">
          <SymptomCard {symptom} onResolve={handleResolve} onDelete={handleDelete} />
        </div>
      {/each}
    {/each}
  {/if}
</div>
