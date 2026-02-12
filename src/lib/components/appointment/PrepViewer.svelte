<!-- L4-02: Prep viewer — tabbed view of patient and professional copies with export. -->
<script lang="ts">
  import type { AppointmentPrep } from '$lib/types/appointment';

  interface Props {
    prep: AppointmentPrep;
    onExport: (type: 'patient' | 'professional' | 'both') => Promise<void>;
    onDone: () => void;
  }
  let { prep, onExport, onDone }: Props = $props();

  let activeTab: 'patient' | 'professional' = $state('patient');
  let exporting = $state(false);

  async function handleExport(type: 'patient' | 'professional' | 'both') {
    exporting = true;
    try {
      await onExport(type);
    } finally {
      exporting = false;
    }
  }
</script>

<div>
  <h2 class="text-xl font-semibold text-stone-800 mb-2">
    Appointment with {prep.professional_name}
  </h2>
  <p class="text-sm text-stone-500 mb-4">{prep.appointment_date}</p>

  <!-- Tab switcher -->
  <div class="flex gap-2 mb-4">
    <button
      class="px-4 py-2 rounded-lg text-sm font-medium min-h-[44px]
             {activeTab === 'patient'
               ? 'bg-[var(--color-primary)] text-white'
               : 'bg-stone-100 text-stone-600'}"
      onclick={() => activeTab = 'patient'}
    >
      Your questions
    </button>
    <button
      class="px-4 py-2 rounded-lg text-sm font-medium min-h-[44px]
             {activeTab === 'professional'
               ? 'bg-[var(--color-primary)] text-white'
               : 'bg-stone-100 text-stone-600'}"
      onclick={() => activeTab = 'professional'}
    >
      Doctor summary
    </button>
  </div>

  <!-- Content -->
  <div class="bg-white rounded-xl p-6 border border-stone-100 shadow-sm mb-4
              max-h-[60vh] overflow-y-auto">
    {#if activeTab === 'patient'}
      <h3 class="font-bold text-stone-800 mb-4">{prep.patient_copy.title}</h3>

      {#if prep.patient_copy.priority_items.length > 0}
        <div class="mb-4 p-3 bg-amber-50 rounded-lg border border-amber-200">
          <h4 class="text-sm font-medium text-amber-800 mb-2">PRIORITY</h4>
          {#each prep.patient_copy.priority_items as item}
            <p class="text-sm text-amber-700 mb-1">{item.text}</p>
            <p class="text-xs text-amber-600">{item.source}</p>
          {/each}
        </div>
      {/if}

      <h4 class="text-sm font-medium text-stone-600 mb-2">YOUR QUESTIONS</h4>
      {#each prep.patient_copy.questions as q, i}
        <div class="mb-3">
          <p class="text-sm text-stone-800">{i + 1}. {q.question}</p>
          <p class="text-xs text-stone-500 mt-0.5">{q.context}</p>
        </div>
      {/each}

      {#if prep.patient_copy.symptoms_to_mention.length > 0}
        <h4 class="text-sm font-medium text-stone-600 mt-4 mb-2">SYMPTOMS TO MENTION</h4>
        {#each prep.patient_copy.symptoms_to_mention as s}
          <p class="text-sm text-stone-700 mb-1">· {s.description}</p>
        {/each}
      {/if}

      {#if prep.patient_copy.medication_changes.length > 0}
        <h4 class="text-sm font-medium text-stone-600 mt-4 mb-2">MEDICATION CHANGES</h4>
        {#each prep.patient_copy.medication_changes as mc}
          <p class="text-sm text-stone-700 mb-1">· {mc.description}</p>
        {/each}
      {/if}

      <p class="text-sm font-medium text-stone-600 mt-6">{prep.patient_copy.reminder}</p>

    {:else}
      <pre class="text-xs text-stone-700 whitespace-pre-wrap font-mono leading-relaxed">{prep.professional_copy.header.title} — {prep.professional_copy.header.date}
{prep.professional_copy.header.professional}
{prep.professional_copy.header.disclaimer}

CURRENT MEDICATIONS:
{#each prep.professional_copy.current_medications as m}{m.name} {m.dose} — {m.frequency} — {m.prescriber}{m.is_recent_change ? ' [CHANGED]' : ''}
{/each}
{#if prep.professional_copy.lab_results.length > 0}
LAB RESULTS:
{#each prep.professional_copy.lab_results as l}{l.test_name}: {l.value} {l.unit} (ref: {l.reference_range}) [{l.abnormal_flag}] — {l.date}
{/each}{/if}
{#if prep.professional_copy.patient_reported_symptoms.length > 0}
PATIENT-REPORTED SYMPTOMS:
{#each prep.professional_copy.patient_reported_symptoms as s}· {s.description} — severity {s.severity}/5 — onset {s.onset_date}
{/each}{/if}</pre>
      <p class="text-xs text-stone-400 mt-4">{prep.professional_copy.disclaimer}</p>
    {/if}
  </div>

  <!-- Export buttons -->
  <div class="flex gap-2">
    <button
      class="flex-1 px-4 py-3 bg-white border border-stone-200 rounded-xl
             text-sm font-medium text-stone-700 min-h-[44px] disabled:opacity-50"
      disabled={exporting}
      onclick={() => handleExport('patient')}
    >
      Print patient copy
    </button>
    <button
      class="flex-1 px-4 py-3 bg-white border border-stone-200 rounded-xl
             text-sm font-medium text-stone-700 min-h-[44px] disabled:opacity-50"
      disabled={exporting}
      onclick={() => handleExport('professional')}
    >
      Print doctor copy
    </button>
  </div>
  <button
    class="w-full mt-2 px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
           text-sm font-medium min-h-[44px] disabled:opacity-50"
    disabled={exporting}
    onclick={() => handleExport('both')}
  >
    Print both
  </button>
  <button
    class="w-full mt-2 px-4 py-3 text-stone-500 text-sm min-h-[44px]"
    onclick={onDone}
  >
    Done
  </button>
</div>
