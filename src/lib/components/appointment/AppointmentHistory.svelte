<!-- L4-02: Appointment history — list of past and upcoming appointments. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { StoredAppointment } from '$lib/types/appointment';
  import Badge from '$lib/components/ui/Badge.svelte';
  import Card from '$lib/components/ui/Card.svelte';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import { ClipboardOutline } from 'flowbite-svelte-icons';

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
    <LoadingState variant="inline" message={$t('common.loading')} />
  {:else if appointments.length === 0}
    <EmptyState
      icon={ClipboardOutline}
      title={$t('appointment.history_empty_title')}
      description={$t('appointment.history_empty_hint')}
      actionLabel={$t('appointment.screen_prepare')}
      onaction={onPrepare}
    />
  {:else}
    {#if upcoming.length > 0}
      <h2 class="text-xs font-medium text-stone-500 dark:text-gray-400 uppercase mb-2">{$t('appointment.history_upcoming')}</h2>
      {#each upcoming as appt (appt.id)}
        <div class="mb-2">
          <Card>
            <div class="flex items-start justify-between">
              <div>
                <p class="font-medium text-stone-800 dark:text-gray-100">
                  {appt.professional_name}
                  {#if appt.professional_specialty}
                    <span class="text-stone-500 dark:text-gray-400 mx-1" aria-hidden="true">&middot;</span>
                    <span class="text-sm text-stone-500 dark:text-gray-400">{appt.professional_specialty}</span>
                  {/if}
                </p>
                <p class="text-sm text-stone-500 dark:text-gray-400">{appt.date}</p>
              </div>
              {#if appt.prep_generated}
                <Badge variant="success" size="sm">{$t('appointment.history_prep_ready')}</Badge>
              {:else}
                <Badge variant="neutral" size="sm">{$t('appointment.history_no_prep')}</Badge>
              {/if}
            </div>
          </Card>
        </div>
      {/each}
    {/if}

    {#if past.length > 0}
      <h2 class="text-xs font-medium text-stone-500 dark:text-gray-400 uppercase mt-4 mb-2">{$t('appointment.history_past')}</h2>
      {#each past as appt (appt.id)}
        <div class="mb-2">
          <Card>
            <div class="flex items-start justify-between">
              <div>
                <p class="font-medium text-stone-800 dark:text-gray-100">
                  {appt.professional_name}
                  {#if appt.professional_specialty}
                    <span class="text-stone-500 dark:text-gray-400 mx-1" aria-hidden="true">&middot;</span>
                    <span class="text-sm text-stone-500 dark:text-gray-400">{appt.professional_specialty}</span>
                  {/if}
                </p>
                <p class="text-sm text-stone-500 dark:text-gray-400">{appt.date}</p>
              </div>
              {#if appt.has_post_notes}
                <Badge variant="success" size="sm">{$t('appointment.history_notes_recorded')}</Badge>
              {:else}
                <button
                  class="text-xs text-[var(--color-interactive)] underline min-h-[44px] px-1"
                  onclick={() => onAddNotes(appt.id)}
                >
                  {$t('appointment.history_add_notes')}
                </button>
              {/if}
            </div>
          </Card>
        </div>
      {/each}
    {/if}
  {/if}
</div>
