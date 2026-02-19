<!-- L4-01: Symptom history list with filters, grouped by date. -->
<script lang="ts">
  import { resolveSymptom, deleteSymptom } from '$lib/api/journal';
  import type { StoredSymptom } from '$lib/types/journal';
  import { CATEGORIES, SEVERITY_LABELS } from '$lib/types/journal';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import Badge from '$lib/components/ui/Badge.svelte';

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
      const dateKey = new Date(s.recorded_date).toLocaleDateString('en-US', {
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
      class="px-3 py-2 rounded-lg border border-stone-200 text-sm text-stone-700
             bg-white min-h-[44px] focus:border-[var(--color-primary)] focus:outline-none"
      bind:value={filterCategory}
    >
      <option value="all">All categories</option>
      {#each CATEGORIES as cat}
        <option value={cat}>{cat}</option>
      {/each}
    </select>

    <label class="flex items-center gap-2 text-sm text-stone-600 min-h-[44px]">
      <input type="checkbox" bind:checked={filterActive}
             class="w-4 h-4 rounded border-stone-300" />
      Active only
    </label>
  </div>

  {#if loading}
    <LoadingState variant="inline" />
  {:else if filtered.length === 0}
    <div class="text-center py-12">
      <p class="text-stone-500 mb-2">No symptoms recorded yet.</p>
      <p class="text-sm text-stone-500">Tap "+ Record" to log how you're feeling.</p>
    </div>
  {:else}
    {#each [...grouped.entries()] as [date, items]}
      <h3 class="text-xs font-medium text-stone-500 uppercase mt-4 mb-2">{date}</h3>
      {#each items as symptom (symptom.id)}
        <div class="bg-white rounded-xl p-4 mb-2 border border-stone-100 shadow-sm">
          <div class="flex items-start justify-between">
            <div>
              <span class="font-medium text-stone-800">{symptom.specific}</span>
              <span class="text-stone-500 mx-1" aria-hidden="true">&middot;</span>
              <span class="text-sm text-stone-500">{SEVERITY_LABELS[symptom.severity] ?? ''}</span>
              <span class="text-stone-500 mx-1" aria-hidden="true">&middot;</span>
              <span class="text-sm text-stone-500">{symptom.category}</span>
            </div>
            <Badge variant={symptom.still_active ? 'success' : 'neutral'} size="sm">
              {symptom.still_active ? 'Active' : 'Resolved'}
            </Badge>
          </div>

          {#if symptom.body_region}
            <p class="text-xs text-stone-500 mt-1">Region: {symptom.body_region}</p>
          {/if}
          {#if symptom.duration}
            <p class="text-xs text-stone-500 mt-0.5">Lasts: {symptom.duration}</p>
          {/if}
          {#if symptom.character}
            <p class="text-xs text-stone-500 mt-0.5">Feels: {symptom.character}</p>
          {/if}
          {#if symptom.related_medication_name}
            <p class="text-xs text-[var(--color-info)] mt-1">
              Note: started {symptom.related_medication_name} recently
            </p>
          {/if}
          {#if symptom.notes}
            <p class="text-xs text-stone-500 mt-1 italic">{symptom.notes}</p>
          {/if}

          <!-- Actions -->
          <div class="flex gap-3 mt-3">
            {#if symptom.still_active}
              <button
                class="text-xs text-stone-500 underline min-h-[44px] px-1"
                onclick={() => handleResolve(symptom.id)}
              >
                Mark resolved
              </button>
            {/if}
            <button
              class="text-xs text-[var(--color-danger)] underline min-h-[44px] px-1"
              onclick={() => handleDelete(symptom.id)}
            >
              Remove
            </button>
          </div>
        </div>
      {/each}
    {/each}
  {/if}
</div>
