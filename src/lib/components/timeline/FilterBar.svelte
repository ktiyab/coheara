<!-- L4-04: Filter bar — type chips, professional dropdown, since last visit. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type {
    EventType, EventCounts, ProfessionalSummary, TimelineEvent,
  } from '$lib/types/timeline';
  import { EVENT_COLORS } from '$lib/utils/timeline';

  interface Props {
    eventCounts: EventCounts;
    professionals: ProfessionalSummary[];
    activeTypes: EventType[];
    selectedProfessionalId: string | null;
    sinceAppointment: string | null;
    completedAppointments: TimelineEvent[];
    onTypeToggle: (types: EventType[]) => void;
    onProfessionalChange: (id: string | null) => void;
    onDateRangeChange: (from: string | null, to: string | null) => void;
    onSinceVisitChange: (appointmentId: string | null) => void;
  }
  let {
    eventCounts, professionals, activeTypes, selectedProfessionalId,
    sinceAppointment, completedAppointments,
    onTypeToggle, onProfessionalChange, onDateRangeChange, onSinceVisitChange,
  }: Props = $props();

  interface ChipDefI18n {
    types: EventType[];
    labelKey: string;
    colorGroup: string;
    countKey: keyof EventCounts;
  }

  const chipDefs: ChipDefI18n[] = [
    { types: ['MedicationStart', 'MedicationStop', 'MedicationDoseChange'], labelKey: 'timeline.filter_meds', colorGroup: 'medication', countKey: 'medications' },
    { types: ['LabResult'], labelKey: 'timeline.filter_labs', colorGroup: 'lab', countKey: 'lab_results' },
    { types: ['Symptom'], labelKey: 'timeline.filter_symptoms', colorGroup: 'symptom', countKey: 'symptoms' },
    { types: ['Procedure'], labelKey: 'timeline.filter_procedures', colorGroup: 'procedure', countKey: 'procedures' },
    { types: ['Appointment'], labelKey: 'timeline.filter_appointments', colorGroup: 'appointment', countKey: 'appointments' },
    { types: ['Document'], labelKey: 'timeline.filter_documents', colorGroup: 'document', countKey: 'documents' },
    { types: ['Diagnosis'], labelKey: 'timeline.filter_diagnoses', colorGroup: 'diagnosis', countKey: 'diagnoses' },
  ];

  function isChipActive(chipTypes: EventType[]): boolean {
    return chipTypes.every(t => activeTypes.includes(t));
  }

  function toggleChip(chipTypes: EventType[]) {
    const allActive = isChipActive(chipTypes);
    let newTypes: EventType[];
    if (allActive) {
      newTypes = activeTypes.filter(t => !chipTypes.includes(t));
    } else {
      newTypes = [...new Set([...activeTypes, ...chipTypes])];
    }
    onTypeToggle(newTypes);
  }

  let showFiltersExpanded = $state(false);
</script>

<div class="px-4 pb-2 border-b border-stone-200 bg-white">
  <!-- Type filter chips (scrollable row) -->
  <div class="flex gap-2 overflow-x-auto pb-2 -mx-1 px-1 scrollbar-hide">
    {#each chipDefs as chip}
      {@const active = isChipActive(chip.types)}
      {@const colors = EVENT_COLORS[chip.colorGroup]}
      {@const label = $t(chip.labelKey)}
      <button
        class="flex items-center gap-1.5 px-3 py-1.5 rounded-full text-sm whitespace-nowrap
               min-h-[36px] transition-colors border
               {active
                 ? 'border-transparent text-stone-800'
                 : 'border-stone-200 text-stone-500 bg-white'}"
        style={active ? `background-color: ${colors.fill}; border-color: ${colors.stroke}40` : ''}
        onclick={() => toggleChip(chip.types)}
        aria-pressed={active}
        aria-label={$t('timeline.filter_events_aria', { values: { label, count: eventCounts[chip.countKey] } })}
      >
        <span class="w-2 h-2 rounded-full"
              style="background-color: {active ? colors.stroke : '#D6D3D1'}"></span>
        {label}
        <span class="text-xs opacity-70">{eventCounts[chip.countKey]}</span>
      </button>
    {/each}
  </div>

  <!-- Expandable filters row -->
  <button
    class="text-xs text-stone-500 py-1 min-h-[44px] w-full text-left"
    onclick={() => { showFiltersExpanded = !showFiltersExpanded; }}
    aria-expanded={showFiltersExpanded}
    aria-controls="timeline-filters-expanded"
  >
    {showFiltersExpanded ? $t('timeline.filter_hide') : $t('timeline.filter_more')}
  </button>

  {#if showFiltersExpanded}
    <div id="timeline-filters-expanded" class="flex flex-wrap gap-3 py-2">
      <!-- Professional dropdown -->
      <div class="flex flex-col gap-1">
        <label for="prof-filter" class="text-xs text-stone-500">{$t('timeline.filter_professional')}</label>
        <select
          id="prof-filter"
          class="text-sm border border-stone-200 rounded-lg px-3 py-2 min-h-[44px]
                 bg-white text-stone-700"
          value={selectedProfessionalId ?? ''}
          onchange={(e) => onProfessionalChange(
            (e.target as HTMLSelectElement).value || null
          )}
        >
          <option value="">{$t('timeline.filter_all_professionals')}</option>
          {#each professionals as prof}
            <option value={prof.id}>
              {prof.name}{prof.specialty ? ` (${prof.specialty})` : ''} — {prof.event_count}
            </option>
          {/each}
        </select>
      </div>

      <!-- Since last visit dropdown -->
      <div class="flex flex-col gap-1">
        <label for="since-visit" class="text-xs text-stone-500">{$t('timeline.filter_since_last_visit')}</label>
        <select
          id="since-visit"
          class="text-sm border border-stone-200 rounded-lg px-3 py-2 min-h-[44px]
                 bg-white text-stone-700"
          value={sinceAppointment ?? ''}
          onchange={(e) => onSinceVisitChange(
            (e.target as HTMLSelectElement).value || null
          )}
        >
          <option value="">{$t('timeline.filter_all_time')}</option>
          {#each completedAppointments as appt}
            <option value={appt.id}>
              {appt.professional_name ?? $t('timeline.filter_visit_fallback')} — {new Date(appt.date).toLocaleDateString()}
            </option>
          {/each}
        </select>
      </div>

      <!-- Date range -->
      <div class="flex flex-col gap-1">
        <label for="date-from" class="text-xs text-stone-500">{$t('timeline.filter_from')}</label>
        <input
          id="date-from"
          type="date"
          class="text-sm border border-stone-200 rounded-lg px-3 py-2 min-h-[44px]
                 bg-white text-stone-700"
          onchange={(e) => onDateRangeChange(
            (e.target as HTMLInputElement).value || null,
            null
          )}
        />
      </div>
      <div class="flex flex-col gap-1">
        <label for="date-to" class="text-xs text-stone-500">{$t('timeline.filter_to')}</label>
        <input
          id="date-to"
          type="date"
          class="text-sm border border-stone-200 rounded-lg px-3 py-2 min-h-[44px]
                 bg-white text-stone-700"
          onchange={(e) => onDateRangeChange(
            null,
            (e.target as HTMLInputElement).value || null
          )}
        />
      </div>
    </div>
  {/if}
</div>
