<!-- L4-04: Event detail popup — displays type-specific details, correlations, actions. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import { invoke } from '@tauri-apps/api/core';
  import type { TimelineEvent, TimelineCorrelation } from '$lib/types/timeline';
  import { EVENT_COLORS, eventColorGroup } from '$lib/utils/timeline';

  import { navigation } from '$lib/stores/navigation.svelte';
  import Badge from '$lib/components/ui/Badge.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import { CloseOutline } from 'flowbite-svelte-icons';

  interface Props {
    event: TimelineEvent;
    correlations: TimelineCorrelation[];
    allEvents?: TimelineEvent[];
    anchor: { x: number; y: number };
    onClose: () => void;
    onScrollToEvent: (eventId: string) => void;
  }
  let { event, correlations, allEvents = [], anchor, onClose, onScrollToEvent }: Props = $props();

  let confirmingDelete = $state(false);
  let actionError = $state('');

  let popupEl: HTMLDivElement | undefined = $state(undefined);

  let popupStyle = $derived(() => {
    const maxWidth = 320;
    const viewportWidth = typeof window !== 'undefined' ? window.innerWidth : 800;
    const viewportHeight = typeof window !== 'undefined' ? window.innerHeight : 600;

    let left = anchor.x + 16;
    let top = anchor.y - 20;

    if (left + maxWidth > viewportWidth - 16) {
      left = anchor.x - maxWidth - 16;
    }
    if (top + 300 > viewportHeight - 16) {
      top = anchor.y - 300;
    }
    left = Math.max(8, left);
    top = Math.max(8, top);

    return `left: ${left}px; top: ${top}px; max-width: ${maxWidth}px;`;
  });

  let colorGroup = $derived(eventColorGroup(event.event_type));
  let colors = $derived(EVENT_COLORS[colorGroup]);

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }

  onMount(() => {
    document.addEventListener('keydown', handleKeydown);
    popupEl?.focus();
    return () => document.removeEventListener('keydown', handleKeydown);
  });

  function formatDate(dateStr: string): string {
    return new Date(dateStr).toLocaleDateString('en-US', {
      weekday: 'long', year: 'numeric', month: 'long', day: 'numeric',
    });
  }

  async function handleResolveSymptom() {
    actionError = '';
    try {
      await invoke('resolve_symptom', { symptomId: event.id });
      onClose();
    } catch (e) {
      actionError = String(e);
    }
  }

  async function handleDeleteSymptom() {
    if (!confirmingDelete) {
      confirmingDelete = true;
      return;
    }
    actionError = '';
    try {
      await invoke('delete_symptom', { symptomId: event.id });
      onClose();
    } catch (e) {
      actionError = String(e);
    }
  }
</script>

<!-- Backdrop (click to close) -->
<button
  class="fixed inset-0 z-20 bg-transparent"
  onclick={onClose}
  aria-label={$t('timeline.event_close_aria')}
  tabindex="-1"
></button>

<!-- Popup card -->
<div
  bind:this={popupEl}
  class="fixed z-30 bg-white dark:bg-gray-900 rounded-xl shadow-xl border border-stone-200 dark:border-gray-700
         overflow-y-auto p-4"
  style={popupStyle()}
  role="dialog"
  aria-label={$t('timeline.event_dialog_aria', { values: { title: event.title } })}
  tabindex="-1"
>
  <!-- Header -->
  <div class="flex items-start gap-2 mb-3">
    <span class="w-3 h-3 rounded-full mt-1 flex-shrink-0"
          style="background-color: {colors.stroke}"></span>
    <div class="flex-1 min-w-0">
      <h3 class="font-medium text-stone-800 dark:text-gray-100 text-sm">{event.title}</h3>
      <p class="text-xs text-stone-500 dark:text-gray-400 mt-0.5">{formatDate(event.date)}</p>
      {#if event.professional_name}
        <p class="text-xs text-stone-500 dark:text-gray-400">{event.professional_name}</p>
      {/if}
    </div>
    <button
      class="text-stone-500 dark:text-gray-400 hover:text-stone-600 dark:hover:text-gray-300 min-h-[44px] min-w-[44px]
             flex items-center justify-center -mr-2 -mt-2"
      onclick={onClose}
      aria-label={$t('timeline.event_close_aria')}
    >
      <CloseOutline class="w-4 h-4" />
    </button>
  </div>

  <!-- Type-specific details -->
  <div class="text-sm text-stone-700 dark:text-gray-200 space-y-1 mb-3">
    {#if event.metadata.kind === 'Medication'}
      <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_dose')}</span> {event.metadata.dose} {event.metadata.frequency}</p>
      {#if event.metadata.brand_name}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_brand')}</span> {event.metadata.brand_name}</p>
      {/if}
      {#if event.metadata.route}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_route')}</span> {event.metadata.route}</p>
      {/if}
      {#if event.metadata.condition}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_condition')}</span> {event.metadata.condition}</p>
      {/if}
      {#if event.metadata.administration_instructions}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_instructions')}</span> {event.metadata.administration_instructions}</p>
      {/if}
      {#if event.metadata.is_otc}
        <Badge variant="neutral" size="sm">{$t('timeline.event_otc')}</Badge>
      {/if}
      {#if event.metadata.reason}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_reason')}</span> {event.metadata.reason}</p>
      {/if}
    {:else if event.metadata.kind === 'DoseChange'}
      <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_changed')}</span> {event.metadata.old_dose ?? '?'} &rarr; {event.metadata.new_dose}</p>
      {#if event.metadata.old_frequency && event.metadata.new_frequency}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_frequency')}</span> {event.metadata.old_frequency} &rarr; {event.metadata.new_frequency}</p>
      {/if}
      {#if event.metadata.reason}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_reason')}</span> {event.metadata.reason}</p>
      {/if}
    {:else if event.metadata.kind === 'Lab'}
      <p>
        <span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_result')}</span>
        {event.metadata.value ?? event.metadata.value_text ?? 'N/A'}
        {event.metadata.unit ?? ''}
      </p>
      {#if event.metadata.reference_low !== null && event.metadata.reference_high !== null}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_range')}</span> {event.metadata.reference_low} — {event.metadata.reference_high} {event.metadata.unit ?? ''}</p>
      {/if}
      {#if event.metadata.abnormal_flag !== 'normal'}
        <p class="text-[var(--color-warning-800)] text-xs">
          {$t('timeline.event_abnormal_warning')}
        </p>
      {/if}
    {:else if event.metadata.kind === 'Symptom'}
      <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_severity')}</span> {event.metadata.severity}/5</p>
      {#if event.metadata.body_region}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_location')}</span> {event.metadata.body_region}</p>
      {/if}
      {#if event.metadata.duration}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_duration')}</span> {event.metadata.duration}</p>
      {/if}
      {#if event.metadata.character}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_character')}</span> {event.metadata.character}</p>
      {/if}
      {#if event.metadata.aggravating}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_aggravating')}</span> {event.metadata.aggravating}</p>
      {/if}
      {#if event.metadata.relieving}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_relieving')}</span> {event.metadata.relieving}</p>
      {/if}
      {#if event.metadata.timing_pattern}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_timing')}</span> {event.metadata.timing_pattern}</p>
      {/if}
      {#if event.metadata.notes}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_notes')}</span> {event.metadata.notes}</p>
      {/if}
      <p>
        <Badge variant={event.metadata.still_active ? 'warning' : 'success'} size="sm">
          {event.metadata.still_active ? $t('timeline.event_still_active') : $t('timeline.event_resolved')}
        </Badge>
      </p>
      {#if event.metadata.resolved_date}
        <p class="text-xs text-stone-500 dark:text-gray-400">{$t('timeline.event_resolved_date')} {event.metadata.resolved_date}</p>
      {/if}
    {:else if event.metadata.kind === 'Procedure'}
      {#if event.metadata.facility}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_facility')}</span> {event.metadata.facility}</p>
      {/if}
      {#if event.metadata.outcome}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_outcome')}</span> {event.metadata.outcome}</p>
      {/if}
      {#if event.metadata.follow_up_required}
        <p class="text-[var(--color-warning-800)] text-xs">{$t('timeline.event_follow_up')}</p>
      {/if}
    {:else if event.metadata.kind === 'Appointment'}
      <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_type')}</span> {event.metadata.appointment_type}</p>
      {#if event.metadata.professional_specialty}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_specialty')}</span> {event.metadata.professional_specialty}</p>
      {/if}
      {#if event.metadata.pre_summary_generated}
        <Badge variant="success" size="sm">{$t('timeline.event_prep_generated')}</Badge>
      {/if}
      {#if event.metadata.post_notes}
        <div class="mt-1 p-2 bg-stone-50 dark:bg-gray-800 rounded-lg text-xs italic text-stone-600 dark:text-gray-300">
          {event.metadata.post_notes}
        </div>
      {/if}
    {:else if event.metadata.kind === 'Document'}
      <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_type')}</span> {event.metadata.document_type}</p>
      <p>
        <Badge variant={event.metadata.verified ? 'success' : 'warning'} size="sm">
          {event.metadata.verified ? $t('timeline.event_verified') : $t('timeline.event_not_verified')}
        </Badge>
      </p>
    {:else if event.metadata.kind === 'Diagnosis'}
      {#if event.metadata.icd_code}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_icd')}</span> {event.metadata.icd_code}</p>
      {/if}
      <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_status')}</span> {event.metadata.status}</p>
    {:else if event.metadata.kind === 'CoherenceAlert'}
      <Badge variant={event.metadata.severity === 'critical' ? 'danger' : event.metadata.severity === 'standard' ? 'warning' : 'neutral'} size="sm">
        {event.metadata.severity}
      </Badge>
      {#if event.metadata.patient_message}
        <p class="mt-1">{event.metadata.patient_message}</p>
      {/if}
      {#if event.metadata.entity_ids.length > 0}
        <p class="text-xs text-stone-500 dark:text-gray-400">
          {$t('timeline.event_entities', { values: { count: event.metadata.entity_ids.length } })}
        </p>
      {/if}
    {:else if event.metadata.kind === 'VitalSign'}
      <p>
        <span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_value')}</span>
        {#if event.metadata.value_secondary !== null}
          {event.metadata.value_primary}/{event.metadata.value_secondary} {event.metadata.unit}
        {:else}
          {event.metadata.value_primary} {event.metadata.unit}
        {/if}
      </p>
      {#if event.metadata.notes}
        <p><span class="text-stone-500 dark:text-gray-400">{$t('timeline.event_notes')}</span> {event.metadata.notes}</p>
      {/if}
      <p class="text-xs text-stone-500 dark:text-gray-400">{$t('timeline.event_source')}: {event.metadata.source}</p>
    {/if}
  </div>

  <!-- Correlations -->
  {#if correlations.length > 0}
    <div class="border-t border-stone-100 dark:border-gray-800 pt-2 mb-3">
      <p class="text-xs text-stone-500 dark:text-gray-400 font-medium mb-1">{$t('timeline.event_related', { values: { count: correlations.length } })}</p>
      {#each correlations as corr}
        <button
          class="w-full text-left text-xs text-stone-600 dark:text-gray-300 py-1.5 hover:text-stone-800 dark:hover:text-gray-100
                 min-h-[44px] flex items-center"
          onclick={() => {
            const targetId = corr.source_id === event.id ? corr.target_id : corr.source_id;
            onScrollToEvent(targetId);
          }}
        >
          <span class="text-stone-500 dark:text-gray-400 mr-1">&rarr;</span>
          {corr.description}
        </button>
      {/each}
    </div>
  {/if}

  <!-- Action buttons -->
  <div class="flex flex-wrap gap-2 border-t border-stone-100 dark:border-gray-800 pt-2">
    {#if event.document_id}
      <Button variant="secondary" size="sm"
        onclick={() => navigation.navigate('document-detail', { documentId: event.document_id! })}>
        {$t('timeline.event_view_document')}
      </Button>
    {/if}

    {#if event.metadata.kind === 'Symptom'}
      <!-- Symptom: navigate to chat, resolve, delete -->
      <Button variant="secondary" size="sm"
        onclick={() => navigation.navigate('chat', { prefill: $t('chat.prefill_symptom') })}>
        {$t('timeline.event_go_to_source')}
      </Button>
      {#if event.metadata.still_active}
        <Button variant="secondary" size="sm" onclick={handleResolveSymptom}>
          {$t('timeline.event_mark_resolved')}
        </Button>
      {/if}
      <Button variant="danger" size="sm" onclick={handleDeleteSymptom}>
        {confirmingDelete ? $t('timeline.event_delete_confirm') : $t('timeline.event_delete_symptom')}
      </Button>
    {:else if event.metadata.kind === 'Medication' || event.metadata.kind === 'DoseChange'}
      <Button variant="secondary" size="sm"
        onclick={() => navigation.navigate('chat', { prefill: $t('timeline.event_ask_med_prefill', { values: { name: event.title } }) })}>
        {$t('timeline.event_ask_ai')}
      </Button>
    {:else if event.metadata.kind === 'Appointment'}
      <Button variant="secondary" size="sm"
        onclick={() => navigation.navigate('chat', { prefill: $t('timeline.event_prepare_prefill', { values: { name: event.professional_name ?? event.title } }) })}>
        {$t('timeline.event_prepare_visit')}
      </Button>
    {:else if event.metadata.kind === 'CoherenceAlert'}
      <Button variant="secondary" size="sm"
        onclick={() => navigation.navigate('chat', { prefill: event.metadata.kind === 'CoherenceAlert' ? (event.metadata.patient_message ?? event.title) : event.title })}>
        {$t('timeline.event_ask_ai')}
      </Button>
    {:else if event.metadata.kind === 'VitalSign'}
      <Button variant="secondary" size="sm"
        onclick={() => navigation.navigate('chat', { prefill: $t('timeline.event_ask_vital_prefill', { values: { type: event.title } }) })}>
        {$t('timeline.event_ask_ai')}
      </Button>
    {:else}
      <Button variant="secondary" size="sm"
        onclick={() => navigation.navigate('chat', { prefill: event.title })}>
        {$t('timeline.event_ask_ai')}
      </Button>
    {/if}
  </div>

  {#if actionError}
    <p class="text-xs text-red-600 dark:text-red-400 mt-1">{actionError}</p>
  {/if}
</div>
