<!-- L4-02: Prep flow — multi-step appointment preparation wizard. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { listProfessionals, prepareAppointment, exportPrepPdf } from '$lib/api/appointment';
  import type { AppointmentPrep, ProfessionalInfo } from '$lib/types/appointment';
  import ProfessionalSelector from './ProfessionalSelector.svelte';
  import PrepViewer from './PrepViewer.svelte';

  interface Props {
    onComplete: () => Promise<void>;
    onCancel: () => void;
  }
  let { onComplete, onCancel }: Props = $props();

  type Step = 'professional' | 'date' | 'generating' | 'viewing';

  let step: Step = $state('professional');
  let professionals: ProfessionalInfo[] = $state([]);
  let selectedProfessionalId: string | null = $state(null);
  let newProfessional: { name: string; specialty: string; institution: string | null } | null = $state(null);
  let appointmentDate = $state('');
  let prep: AppointmentPrep | null = $state(null);
  let error: string | null = $state(null);
  let phiWarning: string | null = $state(null);

  onMount(async () => {
    try {
      professionals = await listProfessionals();
    } catch (e) {
      console.error('Failed to load professionals:', e);
    }
  });

  async function generate() {
    step = 'generating';
    error = null;
    try {
      prep = await prepareAppointment({
        professional_id: selectedProfessionalId,
        new_professional: newProfessional,
        date: appointmentDate,
      });
      step = 'viewing';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      step = 'date';
    }
  }

  async function handleExport(type: 'patient' | 'professional' | 'both') {
    if (!prep) return;
    phiWarning = null;
    try {
      const result = await exportPrepPdf(prep, type);
      phiWarning = result.phi_warning;
    } catch (e) {
      console.error('Export failed:', e);
    }
  }
</script>

<div class="px-6 py-4">
  <button class="text-stone-500 text-sm mb-4 min-h-[44px]" onclick={onCancel}>
    &larr; Cancel
  </button>

  {#if step === 'professional'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">
      Which doctor is this appointment with?
    </h2>
    <ProfessionalSelector
      {professionals}
      onSelect={(id) => { selectedProfessionalId = id; step = 'date'; }}
      onCreateNew={(prof) => { newProfessional = prof; step = 'date'; }}
    />

  {:else if step === 'date'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">When is the appointment?</h2>
    <input
      type="date"
      class="w-full px-4 py-3 rounded-xl border border-stone-200 text-stone-700
             focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)] min-h-[44px]"
      bind:value={appointmentDate}
    />
    {#if error}
      <p class="text-red-600 text-sm mt-2">{error}</p>
    {/if}
    <button
      class="w-full mt-6 px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
             font-medium min-h-[44px] disabled:opacity-50"
      disabled={!appointmentDate}
      onclick={generate}
    >
      Generate preparation
    </button>

  {:else if step === 'generating'}
    <div class="flex flex-col items-center justify-center py-16">
      <div class="animate-spin w-8 h-8 border-2 border-[var(--color-primary)]
                  border-t-transparent rounded-full mb-4"></div>
      <p class="text-stone-500">Preparing your appointment summary...</p>
      <p class="text-xs text-stone-400 mt-1">This may take a few seconds</p>
    </div>

  {:else if step === 'viewing' && prep}
    {#if phiWarning}
      <div class="mb-4 p-4 bg-amber-50 border border-amber-300 rounded-xl" role="alert">
        <p class="text-amber-800 text-sm font-medium">{phiWarning}</p>
      </div>
    {/if}
    <PrepViewer
      {prep}
      onExport={handleExport}
      onDone={onComplete}
    />
  {/if}
</div>
