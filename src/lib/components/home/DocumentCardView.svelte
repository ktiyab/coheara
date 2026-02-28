<!--
  BTL-10 C7: DocumentCardView v2 — iOS Mail/Spark 3-line dense layout.
  Line 1: Status dot + filename (bold) + date
  Line 2: Professional + specialty + document type badge
  Line 3: Entity value preview (or progress bar for processing, or error for failed)

  Status dots:
    ● Green  = Confirmed      ◐ Amber = Pending Review
    ○ Blue   = Processing     ✕ Red   = Failed
-->
<script lang="ts">
  import { t, locale } from 'svelte-i18n';
  import type { DocumentCard } from '$lib/types/home';
  import type { DocumentLifecycleStatus } from '$lib/types/home';
  import Badge from '$lib/components/ui/Badge.svelte';
  import { ChevronRightIcon, RefreshIcon } from '$lib/components/icons/md';

  interface Props {
    card: DocumentCard;
    onTap: (card: DocumentCard) => void;
    onDelete?: (card: DocumentCard) => void;
    onRetry?: (card: DocumentCard) => void;
  }
  let { card, onTap, onDelete, onRetry }: Props = $props();

  // -- Status classification --

  const PROCESSING_STATES: DocumentLifecycleStatus[] = ['Imported', 'Extracting', 'Structuring'];

  let isProcessing = $derived(PROCESSING_STATES.includes(card.status));
  let isFailed = $derived(card.status === 'Failed');
  let isRejected = $derived(card.status === 'Rejected');

  // -- Status dot --

  let statusDot = $derived.by(() => {
    switch (card.status) {
      case 'Confirmed':
        return { char: '\u25CF', class: 'text-[var(--color-success)]' }; // ●
      case 'PendingReview':
        return { char: '\u25D0', class: 'text-amber-500' }; // ◐
      case 'Failed':
      case 'Rejected':
        return { char: '\u2715', class: 'text-red-500' }; // ✕
      default:
        return { char: '\u25CB', class: 'text-blue-500' }; // ○
    }
  });

  // -- Status badge --

  let statusBadge = $derived.by((): { text: string; variant: 'success' | 'warning' | 'danger' | 'info' | 'neutral' } | null => {
    switch (card.status) {
      case 'PendingReview':
        return { text: $t('documents.status_pending_review'), variant: 'warning' };
      case 'Confirmed':
        return { text: $t('documents.status_confirmed'), variant: 'success' };
      case 'Failed':
        return { text: $t('documents.status_failed'), variant: 'danger' };
      case 'Rejected':
        return { text: $t('documents.status_rejected'), variant: 'danger' };
      case 'Extracting':
        return { text: $t('documents.status_extracting'), variant: 'info' };
      case 'Structuring':
        return { text: $t('documents.status_structuring'), variant: 'info' };
      case 'Imported':
        return { text: $t('documents.status_imported'), variant: 'neutral' };
      default:
        return null;
    }
  });

  // -- Entity preview (line 3) --

  let entityText = $derived.by(() => {
    const parts: string[] = [];
    const s = card.entity_summary;
    if (s.medications > 0) parts.push($t('home.card_medications', { values: { count: s.medications } }));
    if (s.lab_results > 0) parts.push($t('home.card_lab_results', { values: { count: s.lab_results } }));
    if (s.diagnoses > 0) parts.push($t('home.card_diagnoses', { values: { count: s.diagnoses } }));
    if (s.allergies > 0) parts.push($t('home.card_allergy_alerts', { values: { count: s.allergies } }));
    if (s.procedures > 0) parts.push($t('home.card_procedures', { values: { count: s.procedures } }));
    if (s.referrals > 0) parts.push($t('home.card_referrals', { values: { count: s.referrals } }));
    return parts.length > 0 ? parts.join(' \u00B7 ') : '';
  });

  // -- Filename display --

  let displayName = $derived(card.source_filename ?? card.document_type);

  function formatDate(dateStr: string | null): string {
    if (!dateStr) return '';
    return new Date(dateStr).toLocaleDateString($locale ?? 'en', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    });
  }
</script>

<button
  class="w-full text-left bg-white dark:bg-gray-900 rounded-xl p-4 shadow-sm border border-stone-100 dark:border-gray-800
         hover:shadow-md transition-shadow min-h-[44px]"
  onclick={() => onTap(card)}
>
  <div class="flex items-start gap-3">
    <!-- Status dot -->
    <span class="shrink-0 text-base leading-6 {statusDot.class}" aria-hidden="true">
      {statusDot.char}
    </span>

    <!-- Content -->
    <div class="flex-1 min-w-0">
      <!-- Line 1: filename + date/badge -->
      <div class="flex items-center justify-between gap-2">
        <span class="font-medium text-sm text-stone-800 dark:text-gray-100 truncate">
          {displayName}
        </span>
        {#if isFailed || isRejected}
          {#if statusBadge}
            <Badge variant={statusBadge.variant} size="sm">{statusBadge.text}</Badge>
          {/if}
        {:else if isProcessing && statusBadge}
          <span class="shrink-0 text-xs text-blue-500 font-medium">{statusBadge.text}</span>
        {:else}
          <span class="shrink-0 text-xs text-stone-500 dark:text-gray-400">
            {formatDate(card.document_date ?? card.imported_at)}
          </span>
        {/if}
      </div>

      <!-- Line 2: professional + type (confirmed/pending) or stage (processing) -->
      {#if !isFailed && !isRejected}
        <p class="text-xs text-stone-500 dark:text-gray-400 mt-1 truncate">
          {#if isProcessing}
            {card.document_type}
          {:else}
            {card.professional_name ?? $t('home.card_unknown_professional')}
            {#if card.professional_specialty}
              &middot; {card.professional_specialty}
            {/if}
            {#if card.document_type}
              &middot; {card.document_type}
            {/if}
          {/if}
        </p>
      {/if}

      <!-- Line 3: entity preview (normal), error (failed), or nothing (processing) -->
      {#if isFailed || isRejected}
        {#if card.error_message}
          <p class="text-xs text-red-500 dark:text-red-400 mt-1 line-clamp-1">
            {card.error_message}
          </p>
        {/if}
        <!-- Action row -->
        <div class="flex items-center gap-2 mt-2">
          {#if isFailed && onRetry}
            <button
              class="inline-flex items-center gap-1 px-2.5 py-1 min-h-[32px] text-xs font-medium
                     text-[var(--color-success)] hover:bg-[var(--color-success-50)] dark:hover:bg-[var(--color-success-900)]/10
                     rounded-lg transition-colors"
              onclick={(e) => { e.stopPropagation(); onRetry(card); }}
            >
              <RefreshIcon class="w-3.5 h-3.5" />
              {$t('common.retry')}
            </button>
          {/if}
          {#if onDelete}
            <button
              class="inline-flex items-center gap-1 px-2.5 py-1 min-h-[32px] text-xs font-medium
                     text-red-600 hover:bg-red-50 dark:hover:bg-red-900/10
                     rounded-lg transition-colors"
              onclick={(e) => { e.stopPropagation(); onDelete(card); }}
            >
              {$t('documents.queue_delete')}
            </button>
          {/if}
        </div>
      {:else if !isProcessing && entityText}
        <p class="text-xs text-stone-500 dark:text-gray-400 mt-1 truncate">
          {entityText}
        </p>
      {:else if isProcessing}
        <!-- No line 3 for processing cards — queue section handles progress -->
      {/if}
    </div>

    <!-- Chevron (only for tappable non-failed cards) -->
    {#if !isFailed && !isRejected}
      <ChevronRightIcon class="w-6 h-6 text-stone-400 dark:text-gray-500 mt-0.5 shrink-0" />
    {/if}
  </div>
</button>
