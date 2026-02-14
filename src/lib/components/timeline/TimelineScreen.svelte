<!-- L4-04: Timeline Screen — container for the full timeline experience. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { getTimelineData } from '$lib/api/timeline';
  import type {
    TimelineData, TimelineFilter, TimelineEvent, TimelineCorrelation,
    ZoomLevel, EventType,
  } from '$lib/types/timeline';
  import { autoSelectZoom } from '$lib/utils/timeline';
  import FilterBar from './FilterBar.svelte';
  import ZoomControls from './ZoomControls.svelte';
  import TimelineCanvas from './TimelineCanvas.svelte';
  import EventDetailPopup from './EventDetailPopup.svelte';
  import EmptyTimeline from './EmptyTimeline.svelte';

  interface Props {
    sinceAppointmentId?: string;
  }
  let { sinceAppointmentId }: Props = $props();

  let timelineData: TimelineData | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);

  let zoom: ZoomLevel = $state('Month');
  let activeTypes: EventType[] = $state([
    'MedicationStart', 'MedicationStop', 'MedicationDoseChange',
    'LabResult', 'Symptom', 'Procedure', 'Appointment', 'Document', 'Diagnosis',
  ]);
  let selectedProfessionalId: string | null = $state(null);
  let dateFrom: string | null = $state(null);
  let dateTo: string | null = $state(null);
  let sinceAppointment: string | null = $state(sinceAppointmentId ?? null);

  let selectedEvent: TimelineEvent | null = $state(null);
  let popupAnchor: { x: number; y: number } | null = $state(null);

  let filter = $derived<TimelineFilter>({
    event_types: activeTypes.length < 9 ? activeTypes : null,
    professional_id: selectedProfessionalId,
    date_from: dateFrom,
    date_to: dateTo,
    since_appointment_id: sinceAppointment,
  });

  let visibleEvents = $derived.by(() => {
    if (!timelineData) return [] as TimelineEvent[];
    return timelineData.events.filter(e => activeTypes.includes(e.event_type));
  });

  let visibleCorrelations = $derived.by(() => {
    if (!timelineData) return [] as TimelineCorrelation[];
    return timelineData.correlations.filter(c => {
      const sourceVisible = visibleEvents.some(e => e.id === c.source_id);
      const targetVisible = visibleEvents.some(e => e.id === c.target_id);
      return sourceVisible && targetVisible;
    });
  });

  let firstLoad = true;

  async function fetchData() {
    try {
      loading = true;
      error = null;
      timelineData = await getTimelineData(filter);

      if (firstLoad && timelineData.date_range.earliest && timelineData.date_range.latest) {
        zoom = autoSelectZoom(
          new Date(timelineData.date_range.earliest),
          new Date(timelineData.date_range.latest),
        );
        firstLoad = false;
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function handleEventTap(event: TimelineEvent, anchor: { x: number; y: number }) {
    selectedEvent = event;
    popupAnchor = anchor;
  }

  function handleClosePopup() {
    selectedEvent = null;
    popupAnchor = null;
  }

  function handleFilterChange(types: EventType[]) {
    activeTypes = types;
  }

  async function handleProfessionalChange(profId: string | null) {
    selectedProfessionalId = profId;
    await fetchData();
  }

  async function handleDateRangeChange(from: string | null, to: string | null) {
    if (from !== null) dateFrom = from;
    if (to !== null) dateTo = to;
    await fetchData();
  }

  async function handleSinceVisitChange(appointmentId: string | null) {
    sinceAppointment = appointmentId;
    await fetchData();
  }

  onMount(() => {
    fetchData();
  });
</script>

<div class="flex flex-col h-full bg-stone-50">
  <!-- Header -->
  <header class="px-4 pt-4 pb-2">
    <h1 class="text-xl font-bold text-stone-800">Timeline</h1>
    <p class="text-sm text-stone-500 mt-0.5">Your medical journey</p>
  </header>

  {#if loading}
    <div class="flex items-center justify-center flex-1">
      <div class="animate-pulse text-stone-400">Loading timeline...</div>
    </div>
  {:else if error}
    <div class="px-6 py-8 text-center">
      <p class="text-red-600 mb-4">Something went wrong: {error}</p>
      <button
        class="px-6 py-3 bg-stone-200 rounded-xl text-stone-700 min-h-[44px]"
        onclick={fetchData}
      >
        Try again
      </button>
    </div>
  {:else if timelineData && timelineData.events.length === 0}
    <EmptyTimeline />
  {:else if timelineData}
    <!-- Filter bar -->
    <FilterBar
      eventCounts={timelineData.event_counts}
      professionals={timelineData.professionals}
      {activeTypes}
      {selectedProfessionalId}
      {sinceAppointment}
      completedAppointments={timelineData.events.filter(
        e => e.event_type === 'Appointment' && e.metadata.kind === 'Appointment' && e.metadata.appointment_type === 'completed'
      )}
      onTypeToggle={handleFilterChange}
      onProfessionalChange={handleProfessionalChange}
      onDateRangeChange={handleDateRangeChange}
      onSinceVisitChange={handleSinceVisitChange}
    />

    <!-- Since last visit banner -->
    {#if sinceAppointment}
      {@const appt = timelineData.events.find(e => e.id === sinceAppointment)}
      {#if appt}
        <div class="mx-4 mb-2 px-4 py-2 bg-teal-50 border border-teal-200 rounded-lg
                    flex items-center justify-between">
          <span class="text-sm text-teal-800">
            Changes since {appt.professional_name ?? 'visit'} on {new Date(appt.date).toLocaleDateString()}
          </span>
          <button
            class="text-sm text-teal-600 font-medium min-h-[44px] min-w-[44px] px-2"
            onclick={() => handleSinceVisitChange(null)}
            aria-label="Clear since last visit filter"
          >
            Clear
          </button>
        </div>
      {/if}
    {/if}

    <!-- Timeline canvas -->
    <div class="flex-1 relative overflow-hidden">
      <TimelineCanvas
        events={visibleEvents}
        correlations={visibleCorrelations}
        dateRange={timelineData.date_range}
        {zoom}
        sinceDate={sinceAppointment
          ? timelineData.events.find(e => e.id === sinceAppointment)?.date ?? null
          : null}
        onEventTap={handleEventTap}
        selectedEventId={selectedEvent?.id ?? null}
      />

      <!-- Zoom controls (floating) -->
      <ZoomControls
        currentZoom={zoom}
        onZoomChange={(z) => { zoom = z; }}
      />

      <!-- Event detail popup -->
      {#if selectedEvent && popupAnchor}
        <EventDetailPopup
          event={selectedEvent}
          correlations={timelineData.correlations.filter(
            c => c.source_id === selectedEvent!.id || c.target_id === selectedEvent!.id
          )}
          anchor={popupAnchor}
          onClose={handleClosePopup}
          onScrollToEvent={(eventId) => {
            handleClosePopup();
            const target = timelineData!.events.find(e => e.id === eventId);
            if (target) handleEventTap(target, { x: 0, y: 0 });
          }}
        />
      {/if}
    </div>
  {/if}
</div>
