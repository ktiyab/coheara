<!-- L4-02: Post-appointment notes — guided note capture after appointment. -->
<script lang="ts">
  import { saveAppointmentNotes } from '$lib/api/appointment';

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
  <button class="text-stone-500 text-sm mb-4 min-h-[44px]" onclick={onCancel}>
    &larr; Cancel
  </button>

  <h2 class="text-xl font-semibold text-stone-800 mb-6">How did the appointment go?</h2>

  <div class="flex flex-col gap-5">
    <label class="flex flex-col gap-1">
      <span class="text-sm font-medium text-stone-700">
        What did the doctor say? <span class="text-red-400">*</span>
      </span>
      <textarea
        bind:value={doctorSaid}
        rows={3}
        placeholder="Main points from the appointment..."
        class="w-full px-4 py-3 rounded-xl border border-stone-200 text-stone-700 text-sm
               focus:outline-none focus:border-[var(--color-primary)] resize-none"
      ></textarea>
    </label>

    <label class="flex flex-col gap-1">
      <span class="text-sm font-medium text-stone-700">
        Any changes to your medications or treatment? <span class="text-red-400">*</span>
      </span>
      <textarea
        bind:value={changesMade}
        rows={3}
        placeholder="New medications, dose changes, stopped treatments..."
        class="w-full px-4 py-3 rounded-xl border border-stone-200 text-stone-700 text-sm
               focus:outline-none focus:border-[var(--color-primary)] resize-none"
      ></textarea>
    </label>

    <label class="flex flex-col gap-1">
      <span class="text-sm font-medium text-stone-700">
        Any follow-up needed? <span class="text-stone-400">(optional)</span>
      </span>
      <textarea
        bind:value={followUp}
        rows={2}
        placeholder="Next appointment, tests to schedule..."
        class="w-full px-4 py-3 rounded-xl border border-stone-200 text-stone-700 text-sm
               focus:outline-none focus:border-[var(--color-primary)] resize-none"
      ></textarea>
    </label>

    <label class="flex flex-col gap-1">
      <span class="text-sm font-medium text-stone-700">
        Anything else you want to note? <span class="text-stone-400">(optional)</span>
      </span>
      <textarea
        bind:value={generalNotes}
        rows={2}
        placeholder="Other observations or reminders..."
        class="w-full px-4 py-3 rounded-xl border border-stone-200 text-stone-700 text-sm
               focus:outline-none focus:border-[var(--color-primary)] resize-none"
      ></textarea>
    </label>

    {#if error}
      <p class="text-red-600 text-sm">{error}</p>
    {/if}

    <button
      class="w-full px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
             font-medium min-h-[44px] disabled:opacity-50"
      disabled={!canSave || saving}
      onclick={handleSave}
    >
      {saving ? 'Saving...' : 'Save notes'}
    </button>
  </div>
</div>
