<script lang="ts">
  import { onMount } from 'svelte';
  import { t, locale } from 'svelte-i18n';
  import { getHomeData, getMoreDocuments, getRecentSymptoms, getExtractionSuggestions, dismissExtractionSuggestion } from '$lib/api/home';
  import { listen } from '@tauri-apps/api/event';
  import { isTauriEnv } from '$lib/utils/tauri';
  import type { HomeData, DocumentCard, RecentSymptomCard, ExtractionSuggestion } from '$lib/types/home';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { profile } from '$lib/stores/profile.svelte';
  import { ai } from '$lib/stores/ai.svelte';
  import DocumentCardView from './DocumentCardView.svelte';
  import ProgressBlock from './ProgressBlock.svelte';
  import FeatureCards from './FeatureCards.svelte';
  import CompanionCard from './CompanionCard.svelte';
  import CriticalAlertBanner from './CriticalAlertBanner.svelte';
  import HealthInsightCards from './HealthInsightCards.svelte';
  import RecentSymptoms from './RecentSymptoms.svelte';
  import ExtractionSuggestions from './ExtractionSuggestions.svelte';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import ErrorState from '$lib/components/ui/ErrorState.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import { getCoherenceAlerts } from '$lib/api/coherence';
  import type { CoherenceAlert } from '$lib/types/coherence';
  import { getCaregiverSummaries, type CaregiverSummary } from '$lib/api/profile';
  import { listAppointments } from '$lib/api/appointment';
  import { getMedications } from '$lib/api/medications';
  import type { StoredAppointment } from '$lib/types/appointment';
  import type { MedicationCard } from '$lib/types/medication';
  import CaregiverDashboard from './CaregiverDashboard.svelte';
  import UpcomingAppointments from './UpcomingAppointments.svelte';
  import ActiveMedsSummary from './ActiveMedsSummary.svelte';
  import DropZoneOverlay from './DropZoneOverlay.svelte';
  import ExtractionReview from './ExtractionReview.svelte';
  import NudgeCard from './NudgeCard.svelte';
  import { extraction } from '$lib/stores/extraction.svelte';
  import { invoke } from '@tauri-apps/api/core';

  let homeData: HomeData | null = $state(null);
  let observations: CoherenceAlert[] = $state([]);
  let dependents: CaregiverSummary[] = $state([]);
  let appointments: StoredAppointment[] = $state([]);
  let activeMeds: MedicationCard[] = $state([]);
  let nudge: { should_nudge: boolean; nudge_type: string | null; message: string | null; related_medication: string | null } | null = $state(null);
  let recentSymptoms: RecentSymptomCard[] = $state([]);
  let suggestions: ExtractionSuggestion[] = $state([]);
  let aiBannerDismissed = $state(false);
  let loading = $state(true);
  let error: string | null = $state(null);
  let loadingMore = $state(false);

  async function refresh() {
    try {
      loading = true;
      error = null;
      const [data, alerts, caregiverData, appts, medData, symptomData, suggestionData] = await Promise.all([
        getHomeData(),
        getCoherenceAlerts().catch(() => [] as CoherenceAlert[]),
        getCaregiverSummaries().catch(() => [] as CaregiverSummary[]),
        listAppointments().catch(() => [] as StoredAppointment[]),
        getMedications({ status: 'Active', prescriber_id: null, search_query: null, include_otc: true })
          .catch(() => ({ medications: [] as MedicationCard[], total_active: 0, total_paused: 0, total_stopped: 0, prescribers: [] })),
        getRecentSymptoms(5).catch(() => [] as RecentSymptomCard[]),
        getExtractionSuggestions().catch(() => [] as ExtractionSuggestion[]),
      ]);
      homeData = data;
      dependents = caregiverData;
      appointments = appts;
      activeMeds = medData.medications;
      recentSymptoms = symptomData;
      suggestions = suggestionData;
      // Show only non-dismissed, standard/info severity observations (exclude critical — handled by CriticalAlertBanner)
      observations = alerts.filter(a => !a.dismissed && a.severity !== 'Critical');
      // LP-01: Refresh pending extraction items
      extraction.refresh().catch(() => {});
      // LP-07: Check for journal nudge
      invoke('check_journal_nudge').then((n) => { nudge = n as typeof nudge; }).catch(() => {});
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function loadMore() {
    if (!homeData || loadingMore) return;
    loadingMore = true;
    try {
      const more = await getMoreDocuments(homeData.recent_documents.length, 20);
      homeData.recent_documents = [...homeData.recent_documents, ...more];
    } catch (e) {
      console.error('Failed to load more documents:', e);
    } finally {
      loadingMore = false;
    }
  }

  function handleDocumentTap(card: DocumentCard) {
    if (card.status === 'PendingReview') {
      navigation.navigate('review', { documentId: card.id });
    } else {
      navigation.navigate('document-detail', { documentId: card.id });
    }
  }

  onMount(() => {
    if (!isTauriEnv()) {
      // Browser-only dev preview: show empty state, no IPC calls
      homeData = {
        stats: { total_documents: 0, documents_pending_review: 0, total_medications: 0, total_lab_results: 0, last_document_date: null, extraction_accuracy: null },
        recent_documents: [],
        onboarding: { first_document_loaded: false, first_document_reviewed: false, first_question_asked: false, three_documents_loaded: false, first_symptom_recorded: false },
        critical_alerts: [],
      };
      loading = false;
      return;
    }
    refresh();
    const unlisten = listen('document-imported', () => refresh());
    return () => { unlisten.then(fn => fn()); };
  });

  function relativeTime(dateStr: string | null): string {
    if (!dateStr) return '';
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    if (diffMins < 1) return $t('home.time_just_now');
    if (diffMins < 60) return $t('home.time_minutes_ago', { values: { count: diffMins } });
    const diffHours = Math.floor(diffMins / 60);
    if (diffHours < 24) return $t('home.time_hours_ago', { values: { count: diffHours } });
    const diffDays = Math.floor(diffHours / 24);
    if (diffDays < 7) return $t('home.time_days_ago', { values: { count: diffDays } });
    return date.toLocaleDateString($locale ?? 'en');
  }
</script>

<!-- Spec 49 [FE-04]: Global drop zone for file import -->
<DropZoneOverlay />

<div class="flex flex-col bg-stone-50 dark:bg-gray-950">
  <!-- Header -->
  <header class="px-[var(--spacing-page-x)] pt-6 pb-4">
    <h1 class="text-2xl font-bold text-stone-800 dark:text-gray-100">
      {$t('home.greeting', { values: { name: profile.name } })}
    </h1>
    {#if homeData && homeData.stats.total_documents > 0}
      <p class="text-sm text-stone-500 dark:text-gray-400 mt-1">
        {$t('home.document_count', { values: { count: homeData.stats.total_documents } })}
        {#if homeData.stats.last_document_date}
          · {$t('home.last_updated', { values: { time: relativeTime(homeData.stats.last_document_date) } })}
        {/if}
      </p>
    {:else if homeData}
      <p class="text-sm text-stone-500 dark:text-gray-400 mt-1">
        {$t('home.greeting_empty_subtitle')}
      </p>
    {/if}
  </header>

  {#if loading}
    <LoadingState message={$t('common.loading')} />
  {:else if error}
    <ErrorState
      message="{$t('home.error')}: {error}"
      onretry={refresh}
    />
  {:else if homeData}
    <!-- ═══ ZONE A: SAFETY ═══ -->
    {#if homeData.critical_alerts.length > 0}
      <CriticalAlertBanner alerts={homeData.critical_alerts} />
    {/if}

    <!-- ═══ ZONE B: ATTENTION (what needs action now) ═══ -->
    <!-- LP-01: Morning review of batch-extracted health data -->
    <ExtractionReview />

    <!-- LP-07: Check-in nudge -->
    {#if nudge}
      <NudgeCard {nudge} />
    {/if}

    <!-- LP-07: Health insight cards (replaces ObservationsBanner) -->
    {#if observations.length > 0}
      <HealthInsightCards alerts={observations} onDismiss={refresh} />
    {/if}

    <!-- LP-07: Extraction suggestions -->
    {#if suggestions.length > 0}
      <ExtractionSuggestions
        {suggestions}
        onDismiss={async (type, entityId) => {
          await dismissExtractionSuggestion(type, entityId).catch(() => {});
          suggestions = suggestions.filter(s => s.id !== `suggestion-${type.split('_')[0]}-${entityId}`);
          // Re-fetch to get accurate list
          getExtractionSuggestions().then(s => { suggestions = s; }).catch(() => {});
        }}
      />
    {/if}

    <!-- ═══ ZONE C: CONTEXT ═══ -->
    {#if dependents.length > 0}
      <CaregiverDashboard {dependents} />
    {/if}

    {#if !homeData.onboarding.first_document_loaded || !homeData.onboarding.first_document_reviewed || !homeData.onboarding.first_question_asked}
      <ProgressBlock progress={homeData.onboarding} />
    {/if}

    {#if homeData.stats.total_documents > 0 && !ai.isAiAvailable && !aiBannerDismissed}
      <div class="mx-[var(--spacing-page-x)] mt-3 p-4 bg-[var(--color-primary-50)] border border-[var(--color-primary-200)] rounded-xl">
        <p class="text-sm font-medium text-[var(--color-text-primary)] mb-1">
          {$t('home.ai_setup_title')}
        </p>
        <p class="text-xs text-[var(--color-text-secondary)] mb-3">
          {$t('home.ai_setup_description')}
        </p>
        <div class="flex gap-2">
          <Button variant="primary" size="sm" onclick={() => navigation.navigate('ai-setup')}>
            {$t('settings.ai_setup')}
          </Button>
          <Button variant="ghost" size="sm" onclick={() => { aiBannerDismissed = true; }}>
            {$t('home.ai_setup_later')}
          </Button>
        </div>
      </div>
    {/if}

    <!-- ═══ ZONE D: STATUS (current health snapshot) ═══ -->
    <RecentSymptoms symptoms={recentSymptoms} />
    <UpcomingAppointments {appointments} />
    <ActiveMedsSummary medications={activeMeds} />

    <!-- Document feed (populated state) -->
    {#if homeData.stats.total_documents > 0}
      <div class="px-[var(--spacing-page-x)] py-3 flex flex-col gap-3">
        {#each homeData.recent_documents as card (card.id)}
          <DocumentCardView {card} onTap={handleDocumentTap} />
        {/each}

        {#if homeData.recent_documents.length < homeData.stats.total_documents}
          <Button variant="ghost" fullWidth loading={loadingMore} onclick={loadMore}>
            {$t('home.load_more')}
          </Button>
        {/if}
      </div>
    {/if}

    <!-- V8-B6: Feature teaching cards -->
    <FeatureCards hasDocuments={homeData.stats.total_documents > 0} />

    <!-- V8-B7: Phone companion card -->
    <CompanionCard />
  {/if}
</div>
