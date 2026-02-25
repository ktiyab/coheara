<!-- E2E-F04: Document detail — read-only view of all extracted entities. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t, locale } from 'svelte-i18n';
  import { getDocumentDetail } from '$lib/api/documents';
  import type { DocumentDetail } from '$lib/types/documents';
  import { navigation } from '$lib/stores/navigation.svelte';
  import BackButton from '$lib/components/ui/BackButton.svelte';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import ErrorState from '$lib/components/ui/ErrorState.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import Badge from '$lib/components/ui/Badge.svelte';

  interface Props {
    documentId: string;
  }
  let { documentId }: Props = $props();

  let detail: DocumentDetail | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);
  let activeSection = $state('overview');

  type Section = { id: string; label: string; count: number };

  let sections = $derived.by((): Section[] => {
    if (!detail) return [];
    const s: Section[] = [{ id: 'overview', label: $t('documents.detail_overview'), count: 0 }];
    if (detail.medications.length > 0) s.push({ id: 'medications', label: $t('documents.detail_medications'), count: detail.medications.length });
    if (detail.lab_results.length > 0) s.push({ id: 'labs', label: $t('documents.detail_lab_results'), count: detail.lab_results.length });
    if (detail.diagnoses.length > 0) s.push({ id: 'diagnoses', label: $t('documents.detail_diagnoses'), count: detail.diagnoses.length });
    if (detail.allergies.length > 0) s.push({ id: 'allergies', label: $t('documents.detail_allergies'), count: detail.allergies.length });
    if (detail.procedures.length > 0) s.push({ id: 'procedures', label: $t('documents.detail_procedures'), count: detail.procedures.length });
    if (detail.referrals.length > 0) s.push({ id: 'referrals', label: $t('documents.detail_referrals'), count: detail.referrals.length });
    return s;
  });

  let totalEntities = $derived.by(() => {
    if (!detail) return 0;
    return detail.medications.length + detail.lab_results.length + detail.diagnoses.length
      + detail.allergies.length + detail.procedures.length + detail.referrals.length;
  });

  async function loadDetail() {
    loading = true;
    error = null;
    try {
      detail = await getDocumentDetail(documentId);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function formatDate(dateStr: string | null): string {
    if (!dateStr) return $t('documents.detail_unknown_date');
    return new Date(dateStr).toLocaleDateString($locale ?? 'en-US', {
      month: 'long', day: 'numeric', year: 'numeric',
    });
  }

  function abnormalColor(flag: string): string {
    switch (flag) {
      case 'critical_low':
      case 'critical_high': return 'text-[var(--color-danger)] bg-[var(--color-danger-50)]';
      case 'low':
      case 'high': return 'text-[var(--color-warning)] bg-[var(--color-warning-50)]';
      default: return 'text-[var(--color-success)] bg-[var(--color-success-50)]';
    }
  }

  function severityColor(severity: string): string {
    switch (severity) {
      case 'life_threatening': return 'text-[var(--color-danger-800)] bg-[var(--color-danger-200)]';
      case 'severe': return 'text-[var(--color-danger)] bg-[var(--color-danger-50)]';
      case 'moderate': return 'text-[var(--color-warning)] bg-[var(--color-warning-50)]';
      default: return 'text-stone-600 dark:text-gray-300 bg-stone-100 dark:bg-gray-800';
    }
  }

  onMount(() => { loadDetail(); });
</script>

<div class="flex flex-col bg-stone-50 dark:bg-gray-950">
  <!-- Header -->
  <header class="flex items-center gap-3 px-4 py-3 bg-stone-50 dark:bg-gray-950 shrink-0">
    <BackButton />
    <div class="flex-1 min-w-0">
      <h1 class="text-lg font-semibold text-stone-800 dark:text-gray-100 truncate">
        {detail?.document_type ?? $t('documents.detail_fallback')}
      </h1>
      {#if detail?.professional_name}
        <p class="text-sm text-stone-500 dark:text-gray-400 truncate">
          {detail.professional_name}
          {#if detail.professional_specialty}
            &middot; {detail.professional_specialty}
          {/if}
        </p>
      {/if}
    </div>
    {#if detail?.status === 'PendingReview'}
      <Button size="sm" onclick={() => navigation.navigate('review', { documentId })}>
        {$t('documents.detail_review')}
      </Button>
    {/if}
  </header>

  {#if loading}
    <LoadingState message={$t('documents.detail_loading')} />

  {:else if error}
    <ErrorState
      message={error}
      onretry={loadDetail}
      retryLabel={$t('documents.detail_try_again')}
    />

  {:else if detail}
    <!-- Section tabs -->
    <div class="flex bg-stone-50 dark:bg-gray-950 overflow-x-auto shrink-0">
      {#each sections as section}
        <button
          class="shrink-0 px-4 py-3 text-sm font-medium min-h-[44px] whitespace-nowrap
                 {activeSection === section.id
                   ? 'text-[var(--color-success)] border-b-2 border-[var(--color-success)]'
                   : 'text-stone-500 dark:text-gray-400'}"
          onclick={() => activeSection = section.id}
        >
          {section.label}
          {#if section.count > 0}
            <span class="ml-1 text-xs opacity-60">({section.count})</span>
          {/if}
        </button>
      {/each}
    </div>

    <!-- Content -->
    <div class="flex-1 overflow-auto px-4 py-4">

      {#if activeSection === 'overview'}
        <!-- Document metadata -->
        <div class="bg-white dark:bg-gray-900 rounded-xl p-4 border border-stone-100 dark:border-gray-800 mb-4">
          <dl class="space-y-3">
            <div class="flex justify-between">
              <dt class="text-sm text-stone-500 dark:text-gray-400">{$t('documents.detail_type')}</dt>
              <dd class="text-sm font-medium text-stone-800 dark:text-gray-100">{detail.document_type}</dd>
            </div>
            <div class="flex justify-between">
              <dt class="text-sm text-stone-500 dark:text-gray-400">{$t('documents.detail_date')}</dt>
              <dd class="text-sm text-stone-800 dark:text-gray-100">{formatDate(detail.document_date)}</dd>
            </div>
            <div class="flex justify-between">
              <dt class="text-sm text-stone-500 dark:text-gray-400">{$t('documents.detail_imported')}</dt>
              <dd class="text-sm text-stone-800 dark:text-gray-100">{formatDate(detail.imported_at)}</dd>
            </div>
            <div class="flex justify-between">
              <dt class="text-sm text-stone-500 dark:text-gray-400">{$t('documents.detail_file')}</dt>
              <dd class="text-sm text-stone-800 dark:text-gray-100 truncate ml-4">{detail.source_filename}</dd>
            </div>
            <div class="flex justify-between">
              <dt class="text-sm text-stone-500 dark:text-gray-400">{$t('documents.detail_status')}</dt>
              <dd class="text-sm">
                <Badge variant={detail.status === 'Confirmed' ? 'success' : 'warning'} size="sm">
                  {detail.status === 'Confirmed' ? $t('documents.detail_confirmed') : $t('documents.detail_pending')}
                </Badge>
              </dd>
            </div>
            {#if detail.ocr_confidence !== null}
              <div class="flex justify-between">
                <dt class="text-sm text-stone-500 dark:text-gray-400">{$t('documents.detail_ocr_confidence')}</dt>
                <dd class="text-sm text-stone-800 dark:text-gray-100">{Math.round(detail.ocr_confidence * 100)}%</dd>
              </div>
            {/if}
            <div class="flex justify-between">
              <dt class="text-sm text-stone-500 dark:text-gray-400">{$t('documents.detail_entities_found')}</dt>
              <dd class="text-sm font-medium text-stone-800 dark:text-gray-100">{totalEntities}</dd>
            </div>
          </dl>
        </div>

        {#if detail.notes}
          <div class="bg-white dark:bg-gray-900 rounded-xl p-4 border border-stone-100 dark:border-gray-800">
            <h3 class="text-sm font-medium text-stone-700 dark:text-gray-200 mb-2">{$t('documents.detail_notes')}</h3>
            <p class="text-sm text-stone-600 dark:text-gray-300">{detail.notes}</p>
          </div>
        {/if}

      {:else if activeSection === 'medications'}
        <div class="space-y-3">
          {#each detail.medications as med}
            <div class="bg-white dark:bg-gray-900 rounded-xl p-4 border border-stone-100 dark:border-gray-800">
              <div class="flex items-start justify-between">
                <div>
                  <p class="font-medium text-stone-800 dark:text-gray-100">{med.generic_name}</p>
                  {#if med.brand_name}
                    <p class="text-xs text-stone-500 dark:text-gray-400">({med.brand_name})</p>
                  {/if}
                </div>
                <span class="text-xs px-2 py-0.5 rounded-full
                             {med.status === 'active' ? 'bg-[var(--color-success-50)] text-[var(--color-success)]' : 'bg-stone-100 dark:bg-gray-800 text-stone-600 dark:text-gray-300'}">
                  {med.status}
                </span>
              </div>
              <div class="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-sm text-stone-600 dark:text-gray-300">
                <span>{med.dose}</span>
                <span>{med.frequency}</span>
                <span>{med.route}</span>
              </div>
              {#if med.start_date || med.end_date}
                <p class="text-xs text-stone-500 dark:text-gray-400 mt-2">
                  {#if med.start_date}{$t('documents.detail_started')} {formatDate(med.start_date)}{/if}
                  {#if med.end_date} &middot; {$t('documents.detail_ended')} {formatDate(med.end_date)}{/if}
                </p>
              {/if}
            </div>
          {/each}
        </div>

      {:else if activeSection === 'labs'}
        <div class="space-y-3">
          {#each detail.lab_results as lab}
            <div class="bg-white dark:bg-gray-900 rounded-xl p-4 border border-stone-100 dark:border-gray-800">
              <div class="flex items-start justify-between">
                <p class="font-medium text-stone-800 dark:text-gray-100">{lab.test_name}</p>
                <span class="text-xs px-2 py-0.5 rounded-full {abnormalColor(lab.abnormal_flag)}">
                  {lab.abnormal_flag === 'normal' ? $t('documents.detail_normal') : lab.abnormal_flag.replace('_', ' ')}
                </span>
              </div>
              <div class="mt-2 flex items-baseline gap-2">
                <span class="text-lg font-semibold text-stone-800 dark:text-gray-100">
                  {lab.value !== null ? lab.value : lab.value_text ?? '—'}
                </span>
                {#if lab.unit}
                  <span class="text-sm text-stone-500 dark:text-gray-400">{lab.unit}</span>
                {/if}
              </div>
              {#if lab.reference_range_low !== null || lab.reference_range_high !== null}
                <p class="text-xs text-stone-500 dark:text-gray-400 mt-1">
                  {$t('documents.detail_reference')} {lab.reference_range_low ?? '—'} – {lab.reference_range_high ?? '—'}
                  {#if lab.unit} {lab.unit}{/if}
                </p>
              {/if}
              <p class="text-xs text-stone-500 dark:text-gray-400 mt-1">{formatDate(lab.collection_date)}</p>
            </div>
          {/each}
        </div>

      {:else if activeSection === 'diagnoses'}
        <div class="space-y-3">
          {#each detail.diagnoses as dx}
            <div class="bg-white dark:bg-gray-900 rounded-xl p-4 border border-stone-100 dark:border-gray-800">
              <div class="flex items-start justify-between">
                <p class="font-medium text-stone-800 dark:text-gray-100">{dx.name}</p>
                <span class="text-xs px-2 py-0.5 rounded-full
                             {dx.status === 'active' ? 'bg-[var(--color-warning-200)] text-[var(--color-warning-800)]'
                               : dx.status === 'resolved' ? 'bg-[var(--color-success-50)] text-[var(--color-success)]'
                               : 'bg-[var(--color-info-200)] text-[var(--color-info)]'}">
                  {dx.status}
                </span>
              </div>
              {#if dx.icd_code}
                <p class="text-xs text-stone-500 dark:text-gray-400 mt-1">{$t('documents.detail_icd')} {dx.icd_code}</p>
              {/if}
              {#if dx.date_diagnosed}
                <p class="text-xs text-stone-500 dark:text-gray-400 mt-1">{$t('documents.detail_diagnosed')} {formatDate(dx.date_diagnosed)}</p>
              {/if}
            </div>
          {/each}
        </div>

      {:else if activeSection === 'allergies'}
        <div class="space-y-3">
          {#each detail.allergies as allergy}
            <div class="bg-white dark:bg-gray-900 rounded-xl p-4 border border-stone-100 dark:border-gray-800">
              <div class="flex items-start justify-between">
                <p class="font-medium text-stone-800 dark:text-gray-100">{allergy.allergen}</p>
                <span class="text-xs px-2 py-0.5 rounded-full {severityColor(allergy.severity)}">
                  {allergy.severity}
                </span>
              </div>
              {#if allergy.reaction}
                <p class="text-sm text-stone-600 dark:text-gray-300 mt-1">{$t('documents.detail_reaction')} {allergy.reaction}</p>
              {/if}
            </div>
          {/each}
        </div>

      {:else if activeSection === 'procedures'}
        <div class="space-y-3">
          {#each detail.procedures as proc}
            <div class="bg-white dark:bg-gray-900 rounded-xl p-4 border border-stone-100 dark:border-gray-800">
              <p class="font-medium text-stone-800 dark:text-gray-100">{proc.name}</p>
              {#if proc.date}
                <p class="text-xs text-stone-500 dark:text-gray-400 mt-1">{formatDate(proc.date)}</p>
              {/if}
              {#if proc.outcome}
                <p class="text-sm text-stone-600 dark:text-gray-300 mt-1">{$t('documents.detail_outcome')} {proc.outcome}</p>
              {/if}
              {#if proc.follow_up_required}
                <span class="inline-block mt-2 text-xs px-2 py-0.5 rounded-full bg-[var(--color-warning-200)] text-[var(--color-warning-800)]">
                  {$t('documents.detail_follow_up')}
                </span>
              {/if}
            </div>
          {/each}
        </div>

      {:else if activeSection === 'referrals'}
        <div class="space-y-3">
          {#each detail.referrals as ref}
            <div class="bg-white dark:bg-gray-900 rounded-xl p-4 border border-stone-100 dark:border-gray-800">
              <div class="flex items-start justify-between">
                <p class="font-medium text-stone-800 dark:text-gray-100">{ref.reason ?? $t('documents.detail_referral_fallback')}</p>
                <span class="text-xs px-2 py-0.5 rounded-full
                             {ref.status === 'completed' ? 'bg-[var(--color-success-50)] text-[var(--color-success)]'
                               : ref.status === 'pending' ? 'bg-[var(--color-warning-200)] text-[var(--color-warning-800)]'
                               : ref.status === 'scheduled' ? 'bg-[var(--color-info-200)] text-[var(--color-info)]'
                               : 'bg-stone-100 dark:bg-gray-800 text-stone-600 dark:text-gray-300'}">
                  {ref.status}
                </span>
              </div>
              <p class="text-xs text-stone-500 dark:text-gray-400 mt-1">{formatDate(ref.date)}</p>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>
