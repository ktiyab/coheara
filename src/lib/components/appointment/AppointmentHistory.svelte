<!-- L4-02: Appointment history — list of past and upcoming appointments. -->
<script lang="ts">
  import type { StoredAppointment } from '$lib/types/appointment';

  interface Props {
    appointments: StoredAppointment[];
    loading: boolean;
    onPrepare: () => void;
    onAddNotes: (id: string) => void;
  }
  let { appointments, loading, onPrepare, onAddNotes }: Props = $props();

  let upcoming = $derived.by(() => {
    const items: StoredAppointment[] = appointments;
    return items.filter(a => a.appointment_type === 'upcoming');
  });

  let past = $derived.by(() => {
    const items: StoredAppointment[] = appointments;
    return items.filter(a => a.appointment_type === 'completed');
  });
</script>

<div class="px-6">
  {#if loading}
    <div class="text-center py-12 text-stone-400">Loading...</div>
  {:else if appointments.length === 0}
    <div class="text-center py-12">
      <p class="text-stone-500 mb-2">No appointments yet.</p>
      <p class="text-sm text-stone-400">Tap "+ Prepare" to get ready for your next visit.</p>
    </div>
  {:else}
    {#if upcoming.length > 0}
      <h3 class="text-xs font-medium text-stone-400 uppercase mb-2">Upcoming</h3>
      {#each upcoming as appt (appt.id)}
        <div class="bg-white rounded-xl p-4 mb-2 border border-stone-100 shadow-sm">
          <div class="flex items-start justify-between">
            <div>
              <p class="font-medium text-stone-800">
                {appt.professional_name}
                {#if appt.professional_specialty}
                  <span class="text-stone-400 mx-1" aria-hidden="true">&middot;</span>
                  <span class="text-sm text-stone-500">{appt.professional_specialty}</span>
                {/if}
              </p>
              <p class="text-sm text-stone-500">{appt.date}</p>
            </div>
            {#if appt.prep_generated}
              <span class="text-xs px-2 py-0.5 rounded-full bg-green-100 text-green-700">
                Prep ready
              </span>
            {:else}
              <span class="text-xs px-2 py-0.5 rounded-full bg-stone-100 text-stone-500">
                No prep
              </span>
            {/if}
          </div>
        </div>
      {/each}
    {/if}

    {#if past.length > 0}
      <h3 class="text-xs font-medium text-stone-400 uppercase mt-4 mb-2">Past</h3>
      {#each past as appt (appt.id)}
        <div class="bg-white rounded-xl p-4 mb-2 border border-stone-100 shadow-sm">
          <div class="flex items-start justify-between">
            <div>
              <p class="font-medium text-stone-800">
                {appt.professional_name}
                {#if appt.professional_specialty}
                  <span class="text-stone-400 mx-1" aria-hidden="true">&middot;</span>
                  <span class="text-sm text-stone-500">{appt.professional_specialty}</span>
                {/if}
              </p>
              <p class="text-sm text-stone-500">{appt.date}</p>
            </div>
            {#if appt.has_post_notes}
              <span class="text-xs px-2 py-0.5 rounded-full bg-green-100 text-green-700">
                Notes recorded
              </span>
            {:else}
              <button
                class="text-xs text-blue-600 underline min-h-[44px] px-1"
                onclick={() => onAddNotes(appt.id)}
              >
                Add notes
              </button>
            {/if}
          </div>
        </div>
      {/each}
    {/if}
  {/if}
</div>
