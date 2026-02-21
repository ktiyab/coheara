<!-- V14: Main journal screen — history + unified entry (replaces 3-view system). -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import { getSymptomHistory, checkJournalNudge } from '$lib/api/journal';
  import type { StoredSymptom, NudgeDecision } from '$lib/types/journal';
  import SymptomEntryPanel from './SymptomEntryPanel.svelte';
  import SymptomHistory from './SymptomHistory.svelte';
  import NudgeBanner from './NudgeBanner.svelte';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import ErrorState from '$lib/components/ui/ErrorState.svelte';
  import Button from '$lib/components/ui/Button.svelte';

  let view: 'history' | 'entry' = $state('history');
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

<div class="flex flex-col bg-stone-50 dark:bg-gray-950">
  <header class="px-6 pt-6 pb-4 flex items-center justify-between">
    <div>
      <h1 class="text-2xl font-bold text-stone-800 dark:text-gray-100">{$t('journal.screen_title')}</h1>
      {#if symptoms.length > 0 && view === 'history'}
        <p class="text-sm text-stone-500 dark:text-gray-400 mt-1">
          {$t('journal.screen_active_symptoms', { values: { count: symptoms.filter(s => s.still_active).length } })}
        </p>
      {/if}
    </div>
    {#if view === 'history'}
      <Button variant="primary" size="sm" onclick={() => { view = 'entry'; }}>
        {$t('journal.entry_btn')}
      </Button>
    {/if}
  </header>

  {#if view === 'entry'}
    <SymptomEntryPanel
      onComplete={async () => { view = 'history'; await refresh(); }}
      onCancel={() => { view = 'history'; }}
    />
  {:else}
    {#if nudge?.should_nudge}
      <NudgeBanner
        {nudge}
        onAccept={() => { view = 'entry'; }}
        onDismiss={() => { nudge = null; }}
      />
    {/if}

    {#if loading && symptoms.length === 0}
      <LoadingState message={$t('journal.screen_loading')} />
    {:else if error}
      <ErrorState
        message="{$t('journal.screen_error_prefix')} {error}"
        onretry={refresh}
        retryLabel={$t('journal.screen_try_again')}
      />
    {:else}
      <SymptomHistory
        {symptoms}
        loading={false}
        onRefresh={refresh}
      />
    {/if}
  {/if}
</div>
