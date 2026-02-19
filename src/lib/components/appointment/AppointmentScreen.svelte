<!-- L4-02: Appointment screen — main container with history/prep/post-notes views. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { onMount } from 'svelte';
  import { listAppointments } from '$lib/api/appointment';
  import type { StoredAppointment } from '$lib/types/appointment';
  import PrepFlow from './PrepFlow.svelte';
  import AppointmentHistory from './AppointmentHistory.svelte';
  import PostNotesFlow from './PostNotesFlow.svelte';
  import ErrorState from '$lib/components/ui/ErrorState.svelte';
  import ScreenHeader from '$lib/components/ui/ScreenHeader.svelte';

  type View = 'history' | 'prep' | 'post-notes';
  let view: View = $state('history');
  let appointments: StoredAppointment[] = $state([]);
  let selectedAppointmentId: string | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);

  async function refresh() {
    loading = true;
    error = null;
    try {
      appointments = await listAppointments();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => { refresh(); });
</script>

<div class="flex flex-col min-h-screen pb-20 bg-stone-50">
  <ScreenHeader
    title={$t('appointment.screen_title')}
    showBack={false}
    actionLabel={view === 'history' ? $t('appointment.screen_prepare') : undefined}
    onaction={view === 'history' ? () => { view = 'prep'; } : undefined}
  />

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
  {:else if error}
    <ErrorState
      message="{$t('common.something_went_wrong')}: {error}"
      onretry={refresh}
      retryLabel={$t('common.try_again')}
    />
  {:else}
    <AppointmentHistory
      {appointments}
      {loading}
      onPrepare={() => view = 'prep'}
      onAddNotes={(id) => { selectedAppointmentId = id; view = 'post-notes'; }}
    />
  {/if}
</div>
