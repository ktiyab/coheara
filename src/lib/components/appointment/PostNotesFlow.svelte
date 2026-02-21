<!-- L4-02: Post-appointment notes — guided note capture after appointment. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { saveAppointmentNotes } from '$lib/api/appointment';
  import BackButton from '$lib/components/ui/BackButton.svelte';
  import Button from '$lib/components/ui/Button.svelte';

  interface Props {
    appointmentId: string;
    onComplete: () => Promise<void>;
    onCancel: () => void;
  }
  let { appointmentId, onComplete, onCancel }: Props = $props();

  let doctorSaid = $state('');
  let changesMade = $state('');
  let followUp = $state('');
  let generalNotes = $state('');
  let saving = $state(false);
  let error: string | null = $state(null);

  let canSave = $derived(doctorSaid.trim().length > 0 && changesMade.trim().length > 0);

  async function handleSave() {
    if (!canSave) return;
    saving = true;
    error = null;
    try {
      await saveAppointmentNotes({
        appointment_id: appointmentId,
        doctor_said: doctorSaid.trim(),
        changes_made: changesMade.trim(),
        follow_up: followUp.trim() || null,
        general_notes: generalNotes.trim() || null,
      });
      await onComplete();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="px-6 py-4">
  <BackButton onclick={onCancel} label={$t('common.cancel')} />

  <h2 class="text-xl font-semibold text-stone-800 dark:text-gray-100 mb-6">{$t('appointment.post_title')}</h2>

  <div class="flex flex-col gap-5">
    <label class="flex flex-col gap-1">
      <span class="text-sm font-medium text-stone-700 dark:text-gray-200">
        {$t('appointment.post_doctor_said_label')} <span class="text-[var(--color-danger)]">*</span>
      </span>
      <textarea
        bind:value={doctorSaid}
        rows={3}
        placeholder={$t('appointment.post_doctor_said_placeholder')}
        class="w-full px-4 py-3 rounded-xl border border-stone-200 dark:border-gray-700 text-stone-700 dark:text-gray-200 text-sm
               dark:bg-gray-900 focus:outline-none focus:border-[var(--color-primary)] resize-none"
      ></textarea>
    </label>

    <label class="flex flex-col gap-1">
      <span class="text-sm font-medium text-stone-700 dark:text-gray-200">
        {$t('appointment.post_changes_label')} <span class="text-[var(--color-danger)]">*</span>
      </span>
      <textarea
        bind:value={changesMade}
        rows={3}
        placeholder={$t('appointment.post_changes_placeholder')}
        class="w-full px-4 py-3 rounded-xl border border-stone-200 dark:border-gray-700 text-stone-700 dark:text-gray-200 text-sm
               dark:bg-gray-900 focus:outline-none focus:border-[var(--color-primary)] resize-none"
      ></textarea>
    </label>

    <label class="flex flex-col gap-1">
      <span class="text-sm font-medium text-stone-700 dark:text-gray-200">
        {$t('appointment.post_follow_up_label')} <span class="text-stone-500 dark:text-gray-400">{$t('common.optional')}</span>
      </span>
      <textarea
        bind:value={followUp}
        rows={2}
        placeholder={$t('appointment.post_follow_up_placeholder')}
        class="w-full px-4 py-3 rounded-xl border border-stone-200 dark:border-gray-700 text-stone-700 dark:text-gray-200 text-sm
               dark:bg-gray-900 focus:outline-none focus:border-[var(--color-primary)] resize-none"
      ></textarea>
    </label>

    <label class="flex flex-col gap-1">
      <span class="text-sm font-medium text-stone-700 dark:text-gray-200">
        {$t('appointment.post_notes_label')} <span class="text-stone-500 dark:text-gray-400">{$t('common.optional')}</span>
      </span>
      <textarea
        bind:value={generalNotes}
        rows={2}
        placeholder={$t('appointment.post_notes_placeholder')}
        class="w-full px-4 py-3 rounded-xl border border-stone-200 dark:border-gray-700 text-stone-700 dark:text-gray-200 text-sm
               dark:bg-gray-900 focus:outline-none focus:border-[var(--color-primary)] resize-none"
      ></textarea>
    </label>

    {#if error}
      <p class="text-[var(--color-danger)] text-sm">{error}</p>
    {/if}

    <Button variant="primary" fullWidth loading={saving} disabled={!canSave} onclick={handleSave}>
      {saving ? $t('common.saving') : $t('appointment.post_save')}
    </Button>
  </div>
</div>
