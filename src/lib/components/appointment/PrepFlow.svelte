<!-- L4-02: Prep flow — multi-step appointment preparation wizard. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { onMount } from 'svelte';
  import { listProfessionals, prepareAppointment, exportPrepPdf } from '$lib/api/appointment';
  import type { AppointmentPrep, ProfessionalInfo } from '$lib/types/appointment';
  import ProfessionalSelector from './ProfessionalSelector.svelte';
  import PrepViewer from './PrepViewer.svelte';
  import BackButton from '$lib/components/ui/BackButton.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';

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
  <div class="mb-4">
    <BackButton onclick={onCancel} label={$t('common.cancel')} />
  </div>

  {#if step === 'professional'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">
      {$t('appointment.prep_flow_select_professional')}
    </h2>
    <ProfessionalSelector
      {professionals}
      onSelect={(id) => { selectedProfessionalId = id; step = 'date'; }}
      onCreateNew={(prof) => { newProfessional = prof; step = 'date'; }}
    />

  {:else if step === 'date'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">{$t('appointment.prep_flow_select_date')}</h2>
    <input
      type="date"
      class="w-full px-4 py-3 rounded-xl border border-stone-200 text-stone-700
             focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)] min-h-[44px]"
      bind:value={appointmentDate}
    />
    {#if error}
      <p class="text-[var(--color-danger)] text-sm mt-2">{error}</p>
    {/if}
    <div class="mt-6">
      <Button variant="primary" fullWidth disabled={!appointmentDate} onclick={generate}>
        {$t('appointment.prep_flow_generate')}
      </Button>
    </div>

  {:else if step === 'generating'}
    <LoadingState message={$t('appointment.prep_flow_generating')} />

  {:else if step === 'viewing' && prep}
    {#if phiWarning}
      <div class="mb-4 p-4 bg-[var(--color-warning-50)] border border-[var(--color-warning-200)] rounded-xl" role="alert">
        <p class="text-[var(--color-warning-800)] text-sm font-medium">{phiWarning}</p>
      </div>
    {/if}
    <PrepViewer
      {prep}
      onExport={handleExport}
      onDone={onComplete}
    />
  {/if}
</div>
