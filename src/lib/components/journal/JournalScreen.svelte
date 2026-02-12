<!-- L4-01: Main journal screen — history + recording toggle + nudge. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { getSymptomHistory, checkJournalNudge } from '$lib/api/journal';
  import type { StoredSymptom, NudgeDecision } from '$lib/types/journal';
  import RecordingFlow from './RecordingFlow.svelte';
  import SymptomHistory from './SymptomHistory.svelte';
  import NudgeBanner from './NudgeBanner.svelte';

  interface Props {
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { onNavigate }: Props = $props();

  let view: 'history' | 'recording' = $state('history');
  let symptoms: StoredSymptom[] = $state([]);
  let nudge: NudgeDecision | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);

  async function refresh() {
    loading = symptoms.length === 0;
    error = null;
    try {
      const [history, nudgeResult] = await Promise.all([
        getSymptomHistory(),
        checkJournalNudge(),
      ]);
      symptoms = history;
      nudge = nudgeResult;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    refresh();
  });
</script>

<div class="flex flex-col min-h-screen pb-20 bg-stone-50">
  <header class="px-6 pt-6 pb-4 flex items-center justify-between">
    <div>
      <h1 class="text-2xl font-bold text-stone-800">Journal</h1>
      {#if symptoms.length > 0}
        <p class="text-sm text-stone-500 mt-1">
          {symptoms.filter(s => s.still_active).length} active symptom{symptoms.filter(s => s.still_active).length === 1 ? '' : 's'}
        </p>
      {/if}
    </div>
    {#if view === 'history'}
      <button
        class="px-4 py-2 bg-[var(--color-primary)] text-white rounded-xl text-sm
               font-medium min-h-[44px]"
        onclick={() => { view = 'recording'; }}
      >
        + Record
      </button>
    {/if}
  </header>

  {#if view === 'recording'}
    <RecordingFlow
      onComplete={async () => { view = 'history'; await refresh(); }}
      onCancel={() => { view = 'history'; }}
    />
  {:else}
    {#if nudge?.should_nudge}
      <NudgeBanner
        {nudge}
        onAccept={() => { view = 'recording'; }}
        onDismiss={() => { nudge = null; }}
      />
    {/if}

    {#if loading && symptoms.length === 0}
      <div class="flex items-center justify-center flex-1">
        <div class="animate-pulse text-stone-400">Loading journal...</div>
      </div>
    {:else if error}
      <div class="px-6 py-8 text-center">
        <p class="text-red-600 mb-4">Something went wrong: {error}</p>
        <button
          class="px-6 py-3 bg-stone-200 rounded-xl text-stone-700 min-h-[44px]"
          onclick={refresh}
        >
          Try again
        </button>
      </div>
    {:else}
      <SymptomHistory
        {symptoms}
        loading={false}
        onRefresh={refresh}
        {onNavigate}
      />
    {/if}
  {/if}
</div>
