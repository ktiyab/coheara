<!-- L3-04: Main review screen — side-by-side original vs. extracted content. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import { getReviewData, getOriginalFile } from '$lib/api/review';
  import type { ReviewData, FieldCorrection, EntitiesStoredSummary } from '$lib/types/review';
  import OriginalViewer from './OriginalViewer.svelte';
  import ExtractedView from './ExtractedView.svelte';
  import ConfidenceSummary from './ConfidenceSummary.svelte';
  import ReviewActions from './ReviewActions.svelte';
  import { navigation } from '$lib/stores/navigation.svelte';
  import ReviewSuccess from './ReviewSuccess.svelte';
  import BackButton from '$lib/components/ui/BackButton.svelte';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import ErrorState from '$lib/components/ui/ErrorState.svelte';

  interface Props {
    documentId: string;
  }
  let { documentId }: Props = $props();

  let reviewData: ReviewData | null = $state(null);
  let originalFileBase64: string | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);
  let corrections: FieldCorrection[] = $state([]);
  let showSuccess = $state(false);
  let confirmResult = $state<{ status: string; entities: EntitiesStoredSummary } | null>(null);

  // Responsive layout
  let windowWidth = $state(1024);
  let activeTab = $state<'original' | 'extracted'>('extracted');

  let isNarrow = $derived(windowWidth < 768);

  // Confidence summary
  let totalFields = $derived.by(() => {
    const data: ReviewData | null = reviewData;
    return data?.extracted_fields?.length ?? 0;
  });
  let flaggedFields = $derived.by(() => {
    const data: ReviewData | null = reviewData;
    if (!data) return 0;
    return data.extracted_fields.filter((f) => f.is_flagged).length;
  });
  let confidentFields = $derived(totalFields - flaggedFields);

  async function loadReviewData() {
    try {
      loading = true;
      error = null;
      reviewData = await getReviewData(documentId);
      originalFileBase64 = await getOriginalFile(documentId);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function handleFieldCorrection(correction: FieldCorrection) {
    corrections = corrections.filter(c => c.field_id !== correction.field_id);
    corrections = [...corrections, correction];
  }

  function handleConfirmSuccess(result: { status: string; entities: EntitiesStoredSummary }) {
    confirmResult = result;
    showSuccess = true;
  }

  onMount(() => {
    loadReviewData();

    function handleResize() {
      windowWidth = window.innerWidth;
    }
    window.addEventListener('resize', handleResize);
    handleResize();
    return () => window.removeEventListener('resize', handleResize);
  });
</script>

{#if showSuccess && confirmResult}
  <ReviewSuccess
    documentType={reviewData?.document_type ?? $t('review.document_fallback')}
    documentDate={reviewData?.document_date}
    status={confirmResult.status}
    entities={confirmResult.entities}
    correctionsApplied={corrections.length}
    onViewDocument={() => navigation.navigate('document-detail', { documentId })}
    onBackToHome={() => navigation.navigate('home')}
    onAskAi={() => {
      const docType = reviewData?.document_type ?? 'document';
      const date = reviewData?.document_date;
      const prefill = date
        ? `Explain my ${docType} from ${date}`
        : `Explain my ${docType}`;
      navigation.navigate('chat', { prefill });
    }}
  />
{:else}
  <div class="flex flex-col h-screen bg-stone-50">
    <!-- Header -->
    <header class="flex items-center gap-3 px-4 py-3 bg-white border-b border-stone-200 shrink-0">
      <BackButton />
      <div class="flex-1 min-w-0">
        <h1 class="text-lg font-semibold text-stone-800 truncate">
          {$t('review.heading_prefix')} {reviewData?.document_type ?? $t('review.document_fallback')}
        </h1>
        {#if reviewData?.professional_name}
          <p class="text-sm text-stone-500 truncate">
            {reviewData.professional_name}
            {#if reviewData.professional_specialty}
              &middot; {reviewData.professional_specialty}
            {/if}
            {#if reviewData.document_date}
              &middot; {reviewData.document_date}
            {/if}
          </p>
        {/if}
      </div>
    </header>

    {#if loading}
      <LoadingState message={$t('review.loading')} />
    {:else if error}
      <ErrorState
        message="{$t('review.error_prefix')} {error}"
        onretry={loadReviewData}
        retryLabel={$t('common.try_again')}
      />
    {:else if reviewData}
      <!-- Tab switcher for narrow screens -->
      {#if isNarrow}
        <div class="flex bg-white border-b border-stone-200 shrink-0">
          <button
            class="flex-1 py-3 text-sm font-medium min-h-[44px]
                   {activeTab === 'original'
                     ? 'text-[var(--color-primary)] border-b-2 border-[var(--color-primary)]'
                     : 'text-stone-500'}"
            onclick={() => activeTab = 'original'}
          >
            {$t('review.tab_original')}
          </button>
          <button
            class="flex-1 py-3 text-sm font-medium min-h-[44px]
                   {activeTab === 'extracted'
                     ? 'text-[var(--color-primary)] border-b-2 border-[var(--color-primary)]'
                     : 'text-stone-500'}"
            onclick={() => activeTab = 'extracted'}
          >
            {$t('review.tab_extracted')} ({corrections.length > 0 ? $t('review.tab_corrected_count', { values: { count: corrections.length } }) : $t('review.tab_review_suffix')})
          </button>
        </div>
      {/if}

      <!-- Side-by-side / tabbed content -->
      <div class="flex-1 overflow-hidden {isNarrow ? '' : 'flex'}">
        {#if !isNarrow || activeTab === 'original'}
          <div class="{isNarrow ? 'h-full' : 'w-[45%] min-w-[300px]'} border-r border-stone-200 overflow-auto">
            <OriginalViewer
              fileBase64={originalFileBase64}
              fileType={reviewData.original_file_type}
            />
          </div>
        {/if}

        {#if !isNarrow || activeTab === 'extracted'}
          <div class="{isNarrow ? 'h-full' : 'flex-1 min-w-[300px]'} overflow-auto pb-40">
            <ExtractedView
              fields={reviewData.extracted_fields}
              warnings={reviewData.plausibility_warnings}
              {corrections}
              onCorrection={handleFieldCorrection}
            />
          </div>
        {/if}
      </div>

      <!-- Confidence summary bar -->
      <ConfidenceSummary
        {totalFields}
        {confidentFields}
        {flaggedFields}
        overallConfidence={reviewData.overall_confidence}
      />

      <!-- Action bar -->
      <ReviewActions
        {documentId}
        {corrections}
        {flaggedFields}
        onConfirmSuccess={handleConfirmSuccess}
        onReject={() => navigation.goBack()}
      />
    {/if}
  </div>
{/if}
