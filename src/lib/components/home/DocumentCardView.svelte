<script lang="ts">
  import { t, locale } from 'svelte-i18n';
  import type { DocumentCard } from '$lib/types/home';
  import Badge from '$lib/components/ui/Badge.svelte';
  import { ChevronRightOutline } from 'flowbite-svelte-icons';

  interface Props {
    card: DocumentCard;
    onTap: (card: DocumentCard) => void;
  }
  let { card, onTap }: Props = $props();

  let entityText = $derived.by(() => {
    const parts: string[] = [];
    const s = card.entity_summary;
    if (s.medications > 0) parts.push($t('home.card_medications', { values: { count: s.medications } }));
    if (s.lab_results > 0) parts.push($t('home.card_lab_results', { values: { count: s.lab_results } }));
    if (s.diagnoses > 0) parts.push($t('home.card_diagnoses', { values: { count: s.diagnoses } }));
    if (s.allergies > 0) parts.push($t('home.card_allergy_alerts', { values: { count: s.allergies } }));
    if (s.procedures > 0) parts.push($t('home.card_procedures', { values: { count: s.procedures } }));
    if (s.referrals > 0) parts.push($t('home.card_referrals', { values: { count: s.referrals } }));
    return parts.length > 0 ? parts.join(' · ') : $t('home.card_processing');
  });

  let statusBadge = $derived.by(() => {
    if (card.status === 'PendingReview') {
      return { text: $t('home.card_pending_review'), variant: 'warning' as const };
    }
    return { text: $t('home.card_confirmed'), variant: 'success' as const };
  });

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
  <div class="flex items-start justify-between gap-3">
    <div class="flex-1 min-w-0">
      <div class="flex items-center gap-2">
        <span class="font-medium text-stone-800 dark:text-gray-100 truncate">{card.document_type}</span>
        <Badge variant={statusBadge.variant} size="sm">
          {statusBadge.text}
        </Badge>
      </div>
      <p class="text-sm text-stone-500 dark:text-gray-400 mt-1 truncate">
        {card.professional_name ?? $t('home.card_unknown_professional')}
        {#if card.professional_specialty}
          · {card.professional_specialty}
        {/if}
      </p>
      <p class="text-xs text-stone-500 dark:text-gray-400 mt-1">
        {formatDate(card.document_date ?? card.imported_at)}
        · {entityText}
      </p>
    </div>
    <ChevronRightOutline class="w-4 h-4 text-stone-300 dark:text-gray-600 mt-1" />
  </div>
</button>
