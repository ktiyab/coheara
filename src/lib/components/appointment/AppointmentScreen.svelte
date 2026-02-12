<!-- L4-02: Appointment screen — main container with history/prep/post-notes views. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { listAppointments } from '$lib/api/appointment';
  import type { StoredAppointment } from '$lib/types/appointment';
  import PrepFlow from './PrepFlow.svelte';
  import AppointmentHistory from './AppointmentHistory.svelte';
  import PostNotesFlow from './PostNotesFlow.svelte';

  interface Props {
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { onNavigate }: Props = $props();

  type View = 'history' | 'prep' | 'post-notes';
  let view: View = $state('history');
  let appointments: StoredAppointment[] = $state([]);
  let selectedAppointmentId: string | null = $state(null);
  let loading = $state(true);

  async function refresh() {
    loading = true;
    try {
      appointments = await listAppointments();
    } catch (e) {
      console.error('Failed to load appointments:', e);
    } finally {
      loading = false;
    }
  }

  onMount(() => { refresh(); });
</script>

<div class="flex flex-col min-h-screen pb-20 bg-stone-50">
  <header class="px-6 pt-6 pb-4 flex items-center justify-between">
    <h1 class="text-2xl font-bold text-stone-800">Appointments</h1>
    {#if view === 'history'}
      <button
        class="px-4 py-2 bg-[var(--color-primary)] text-white rounded-xl text-sm
               font-medium min-h-[44px]"
        onclick={() => view = 'prep'}
      >
        + Prepare
      </button>
    {/if}
  </header>

  {#if view === 'prep'}
    <PrepFlow
      onComplete={async () => { view = 'history'; await refresh(); }}
      onCancel={() => view = 'history'}
    />
  {:else if view === 'post-notes' && selectedAppointmentId}
    <PostNotesFlow
      appointmentId={selectedAppointmentId}
      onComplete={async () => { view = 'history'; await refresh(); }}
      onCancel={() => view = 'history'}
    />
  {:else}
    <AppointmentHistory
      {appointments}
      {loading}
      onPrepare={() => view = 'prep'}
      onAddNotes={(id) => { selectedAppointmentId = id; view = 'post-notes'; }}
      {onNavigate}
    />
  {/if}
</div>
