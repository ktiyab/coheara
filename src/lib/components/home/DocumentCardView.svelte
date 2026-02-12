<script lang="ts">
  import type { DocumentCard } from '$lib/types/home';

  interface Props {
    card: DocumentCard;
    onTap: (card: DocumentCard) => void;
  }
  let { card, onTap }: Props = $props();

  let entityText = $derived.by(() => {
    const parts: string[] = [];
    const s = card.entity_summary;
    if (s.medications > 0) parts.push(`${s.medications} medication${s.medications > 1 ? 's' : ''}`);
    if (s.lab_results > 0) parts.push(`${s.lab_results} lab result${s.lab_results > 1 ? 's' : ''}`);
    if (s.diagnoses > 0) parts.push(`${s.diagnoses} diagnosis${s.diagnoses > 1 ? 'es' : ''}`);
    if (s.allergies > 0) parts.push(`${s.allergies} allergy${s.allergies > 1 ? ' alerts' : ' alert'}`);
    if (s.procedures > 0) parts.push(`${s.procedures} procedure${s.procedures > 1 ? 's' : ''}`);
    if (s.referrals > 0) parts.push(`${s.referrals} referral${s.referrals > 1 ? 's' : ''}`);
    return parts.length > 0 ? parts.join(' · ') : 'Processing...';
  });

  let statusBadge = $derived.by(() => {
    if (card.status === 'PendingReview') {
      return { text: 'Pending review', color: 'bg-amber-100 text-amber-700' };
    }
    return { text: 'Confirmed', color: 'bg-green-100 text-green-700' };
  });

  function formatDate(dateStr: string | null): string {
    if (!dateStr) return '';
    return new Date(dateStr).toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    });
  }
</script>

<button
  class="w-full text-left bg-white rounded-xl p-4 shadow-sm border border-stone-100
         hover:shadow-md transition-shadow min-h-[44px]"
  onclick={() => onTap(card)}
>
  <div class="flex items-start justify-between gap-3">
    <div class="flex-1 min-w-0">
      <div class="flex items-center gap-2">
        <span class="font-medium text-stone-800 truncate">{card.document_type}</span>
        <span class="text-xs px-2 py-0.5 rounded-full {statusBadge.color}">
          {statusBadge.text}
        </span>
      </div>
      <p class="text-sm text-stone-500 mt-1 truncate">
        {card.professional_name ?? 'Unknown professional'}
        {#if card.professional_specialty}
          · {card.professional_specialty}
        {/if}
      </p>
      <p class="text-xs text-stone-400 mt-1">
        {formatDate(card.document_date ?? card.imported_at)}
        · {entityText}
      </p>
    </div>
    <span class="text-stone-300 mt-1" aria-hidden="true">&rsaquo;</span>
  </div>
</button>
