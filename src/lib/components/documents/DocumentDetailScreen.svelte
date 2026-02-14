<!-- E2E-F04: Document detail — read-only view of all extracted entities. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { getDocumentDetail } from '$lib/api/documents';
  import type { DocumentDetail } from '$lib/types/documents';
  import { navigation } from '$lib/stores/navigation.svelte';

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
    const s: Section[] = [{ id: 'overview', label: 'Overview', count: 0 }];
    if (detail.medications.length > 0) s.push({ id: 'medications', label: 'Medications', count: detail.medications.length });
    if (detail.lab_results.length > 0) s.push({ id: 'labs', label: 'Lab Results', count: detail.lab_results.length });
    if (detail.diagnoses.length > 0) s.push({ id: 'diagnoses', label: 'Diagnoses', count: detail.diagnoses.length });
    if (detail.allergies.length > 0) s.push({ id: 'allergies', label: 'Allergies', count: detail.allergies.length });
    if (detail.procedures.length > 0) s.push({ id: 'procedures', label: 'Procedures', count: detail.procedures.length });
    if (detail.referrals.length > 0) s.push({ id: 'referrals', label: 'Referrals', count: detail.referrals.length });
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
    if (!dateStr) return 'Unknown';
    return new Date(dateStr).toLocaleDateString('en-US', {
      month: 'long', day: 'numeric', year: 'numeric',
    });
  }

  function abnormalColor(flag: string): string {
    switch (flag) {
      case 'critical_low':
      case 'critical_high': return 'text-red-600 bg-red-50';
      case 'low':
      case 'high': return 'text-amber-600 bg-amber-50';
      default: return 'text-green-600 bg-green-50';
    }
  }

  function severityColor(severity: string): string {
    switch (severity) {
      case 'life_threatening': return 'text-red-700 bg-red-100';
      case 'severe': return 'text-red-600 bg-red-50';
      case 'moderate': return 'text-amber-600 bg-amber-50';
      default: return 'text-stone-600 bg-stone-100';
    }
  }

  onMount(() => { loadDetail(); });
</script>

<div class="flex flex-col min-h-screen bg-stone-50">
  <!-- Header -->
  <header class="flex items-center gap-3 px-4 py-3 bg-white border-b border-stone-200 shrink-0">
    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center
             text-stone-500 hover:text-stone-700"
      onclick={() => navigation.goBack()}
      aria-label="Go back"
    >
      &larr;
    </button>
    <div class="flex-1 min-w-0">
      <h1 class="text-lg font-semibold text-stone-800 truncate">
        {detail?.document_type ?? 'Document'}
      </h1>
      {#if detail?.professional_name}
        <p class="text-sm text-stone-500 truncate">
          {detail.professional_name}
          {#if detail.professional_specialty}
            &middot; {detail.professional_specialty}
          {/if}
        </p>
      {/if}
    </div>
    {#if detail?.status === 'PendingReview'}
      <button
        class="px-3 py-1.5 bg-amber-100 text-amber-700 rounded-lg text-xs font-medium min-h-[44px]"
        onclick={() => navigation.navigate('review', { documentId })}
      >
        Review
      </button>
    {/if}
  </header>

  {#if loading}
    <div class="flex items-center justify-center flex-1">
      <div class="animate-pulse text-stone-400">Loading document...</div>
    </div>

  {:else if error}
    <div class="flex flex-col items-center justify-center flex-1 px-6 text-center">
      <p class="text-red-600 mb-4">{error}</p>
      <button
        class="px-6 py-3 bg-stone-200 rounded-xl text-stone-700 min-h-[44px]"
        onclick={loadDetail}
      >
        Try again
      </button>
    </div>

  {:else if detail}
    <!-- Section tabs -->
    <div class="flex bg-white border-b border-stone-200 overflow-x-auto shrink-0">
      {#each sections as section}
        <button
          class="shrink-0 px-4 py-3 text-sm font-medium min-h-[44px] whitespace-nowrap
                 {activeSection === section.id
                   ? 'text-[var(--color-primary)] border-b-2 border-[var(--color-primary)]'
                   : 'text-stone-500'}"
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
    <div class="flex-1 overflow-auto px-4 py-4 pb-20">

      {#if activeSection === 'overview'}
        <!-- Document metadata -->
        <div class="bg-white rounded-xl p-4 border border-stone-100 mb-4">
          <dl class="space-y-3">
            <div class="flex justify-between">
              <dt class="text-sm text-stone-500">Type</dt>
              <dd class="text-sm font-medium text-stone-800">{detail.document_type}</dd>
            </div>
            <div class="flex justify-between">
              <dt class="text-sm text-stone-500">Date</dt>
              <dd class="text-sm text-stone-800">{formatDate(detail.document_date)}</dd>
            </div>
            <div class="flex justify-between">
              <dt class="text-sm text-stone-500">Imported</dt>
              <dd class="text-sm text-stone-800">{formatDate(detail.imported_at)}</dd>
            </div>
            <div class="flex justify-between">
              <dt class="text-sm text-stone-500">File</dt>
              <dd class="text-sm text-stone-800 truncate ml-4">{detail.source_filename}</dd>
            </div>
            <div class="flex justify-between">
              <dt class="text-sm text-stone-500">Status</dt>
              <dd class="text-sm">
                <span class="px-2 py-0.5 rounded-full text-xs font-medium
                             {detail.status === 'Confirmed'
                               ? 'bg-green-100 text-green-700'
                               : 'bg-amber-100 text-amber-700'}">
                  {detail.status === 'Confirmed' ? 'Confirmed' : 'Pending Review'}
                </span>
              </dd>
            </div>
            {#if detail.ocr_confidence !== null}
              <div class="flex justify-between">
                <dt class="text-sm text-stone-500">OCR Confidence</dt>
                <dd class="text-sm text-stone-800">{Math.round(detail.ocr_confidence * 100)}%</dd>
              </div>
            {/if}
            <div class="flex justify-between">
              <dt class="text-sm text-stone-500">Entities Found</dt>
              <dd class="text-sm font-medium text-stone-800">{totalEntities}</dd>
            </div>
          </dl>
        </div>

        {#if detail.notes}
          <div class="bg-white rounded-xl p-4 border border-stone-100">
            <h3 class="text-sm font-medium text-stone-700 mb-2">Notes</h3>
            <p class="text-sm text-stone-600">{detail.notes}</p>
          </div>
        {/if}

      {:else if activeSection === 'medications'}
        <div class="space-y-3">
          {#each detail.medications as med}
            <div class="bg-white rounded-xl p-4 border border-stone-100">
              <div class="flex items-start justify-between">
                <div>
                  <p class="font-medium text-stone-800">{med.generic_name}</p>
                  {#if med.brand_name}
                    <p class="text-xs text-stone-500">({med.brand_name})</p>
                  {/if}
                </div>
                <span class="text-xs px-2 py-0.5 rounded-full
                             {med.status === 'active' ? 'bg-green-100 text-green-700' : 'bg-stone-100 text-stone-600'}">
                  {med.status}
                </span>
              </div>
              <div class="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-sm text-stone-600">
                <span>{med.dose}</span>
                <span>{med.frequency}</span>
                <span>{med.route}</span>
              </div>
              {#if med.start_date || med.end_date}
                <p class="text-xs text-stone-400 mt-2">
                  {#if med.start_date}Started {formatDate(med.start_date)}{/if}
                  {#if med.end_date} &middot; Ended {formatDate(med.end_date)}{/if}
                </p>
              {/if}
            </div>
          {/each}
        </div>

      {:else if activeSection === 'labs'}
        <div class="space-y-3">
          {#each detail.lab_results as lab}
            <div class="bg-white rounded-xl p-4 border border-stone-100">
              <div class="flex items-start justify-between">
                <p class="font-medium text-stone-800">{lab.test_name}</p>
                <span class="text-xs px-2 py-0.5 rounded-full {abnormalColor(lab.abnormal_flag)}">
                  {lab.abnormal_flag === 'normal' ? 'Normal' : lab.abnormal_flag.replace('_', ' ')}
                </span>
              </div>
              <div class="mt-2 flex items-baseline gap-2">
                <span class="text-lg font-semibold text-stone-800">
                  {lab.value !== null ? lab.value : lab.value_text ?? '—'}
                </span>
                {#if lab.unit}
                  <span class="text-sm text-stone-500">{lab.unit}</span>
                {/if}
              </div>
              {#if lab.reference_range_low !== null || lab.reference_range_high !== null}
                <p class="text-xs text-stone-400 mt-1">
                  Reference: {lab.reference_range_low ?? '—'} – {lab.reference_range_high ?? '—'}
                  {#if lab.unit} {lab.unit}{/if}
                </p>
              {/if}
              <p class="text-xs text-stone-400 mt-1">{formatDate(lab.collection_date)}</p>
            </div>
          {/each}
        </div>

      {:else if activeSection === 'diagnoses'}
        <div class="space-y-3">
          {#each detail.diagnoses as dx}
            <div class="bg-white rounded-xl p-4 border border-stone-100">
              <div class="flex items-start justify-between">
                <p class="font-medium text-stone-800">{dx.name}</p>
                <span class="text-xs px-2 py-0.5 rounded-full
                             {dx.status === 'active' ? 'bg-amber-100 text-amber-700'
                               : dx.status === 'resolved' ? 'bg-green-100 text-green-700'
                               : 'bg-blue-100 text-blue-700'}">
                  {dx.status}
                </span>
              </div>
              {#if dx.icd_code}
                <p class="text-xs text-stone-500 mt-1">ICD: {dx.icd_code}</p>
              {/if}
              {#if dx.date_diagnosed}
                <p class="text-xs text-stone-400 mt-1">Diagnosed {formatDate(dx.date_diagnosed)}</p>
              {/if}
            </div>
          {/each}
        </div>

      {:else if activeSection === 'allergies'}
        <div class="space-y-3">
          {#each detail.allergies as allergy}
            <div class="bg-white rounded-xl p-4 border border-stone-100">
              <div class="flex items-start justify-between">
                <p class="font-medium text-stone-800">{allergy.allergen}</p>
                <span class="text-xs px-2 py-0.5 rounded-full {severityColor(allergy.severity)}">
                  {allergy.severity}
                </span>
              </div>
              {#if allergy.reaction}
                <p class="text-sm text-stone-600 mt-1">Reaction: {allergy.reaction}</p>
              {/if}
            </div>
          {/each}
        </div>

      {:else if activeSection === 'procedures'}
        <div class="space-y-3">
          {#each detail.procedures as proc}
            <div class="bg-white rounded-xl p-4 border border-stone-100">
              <p class="font-medium text-stone-800">{proc.name}</p>
              {#if proc.date}
                <p class="text-xs text-stone-500 mt-1">{formatDate(proc.date)}</p>
              {/if}
              {#if proc.outcome}
                <p class="text-sm text-stone-600 mt-1">Outcome: {proc.outcome}</p>
              {/if}
              {#if proc.follow_up_required}
                <span class="inline-block mt-2 text-xs px-2 py-0.5 rounded-full bg-amber-100 text-amber-700">
                  Follow-up required
                </span>
              {/if}
            </div>
          {/each}
        </div>

      {:else if activeSection === 'referrals'}
        <div class="space-y-3">
          {#each detail.referrals as ref}
            <div class="bg-white rounded-xl p-4 border border-stone-100">
              <div class="flex items-start justify-between">
                <p class="font-medium text-stone-800">{ref.reason ?? 'Referral'}</p>
                <span class="text-xs px-2 py-0.5 rounded-full
                             {ref.status === 'completed' ? 'bg-green-100 text-green-700'
                               : ref.status === 'pending' ? 'bg-amber-100 text-amber-700'
                               : ref.status === 'scheduled' ? 'bg-blue-100 text-blue-700'
                               : 'bg-stone-100 text-stone-600'}">
                  {ref.status}
                </span>
              </div>
              <p class="text-xs text-stone-400 mt-1">{formatDate(ref.date)}</p>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>
