<!-- L4-02: Professional selector — choose existing or create new. -->
<script lang="ts">
  import type { ProfessionalInfo } from '$lib/types/appointment';
  import { SPECIALTIES } from '$lib/types/appointment';

  interface Props {
    professionals: ProfessionalInfo[];
    onSelect: (id: string) => void;
    onCreateNew: (prof: { name: string; specialty: string; institution: string | null }) => void;
  }
  let { professionals, onSelect, onCreateNew }: Props = $props();

  let showNewForm = $state(false);
  let newName = $state('');
  let newSpecialty = $state('GP');
  let newInstitution = $state('');

  function handleCreate() {
    if (!newName.trim()) return;
    onCreateNew({
      name: newName.trim(),
      specialty: newSpecialty,
      institution: newInstitution.trim() || null,
    });
  }
</script>

<div class="flex flex-col gap-3">
  {#if !showNewForm}
    {#each professionals as prof}
      <button
        class="w-full text-left p-4 bg-white rounded-xl border border-stone-100 shadow-sm
               hover:border-[var(--color-primary)] transition-colors min-h-[44px]"
        onclick={() => onSelect(prof.id)}
      >
        <p class="font-medium text-stone-800">{prof.name}</p>
        <p class="text-sm text-stone-500">
          {prof.specialty ?? 'Specialist'}
          {#if prof.last_seen_date}
            <span class="text-stone-400"> · Last visit: {prof.last_seen_date}</span>
          {/if}
        </p>
        {#if prof.institution}
          <p class="text-xs text-stone-400 mt-0.5">{prof.institution}</p>
        {/if}
      </button>
    {/each}

    {#if professionals.length === 0}
      <p class="text-center text-stone-400 py-4">No professionals found. Add one below.</p>
    {/if}

    <button
      class="w-full p-4 border-2 border-dashed border-stone-200 rounded-xl
             text-stone-500 text-sm font-medium min-h-[44px] hover:border-stone-300
             transition-colors"
      onclick={() => showNewForm = true}
    >
      + Add new professional
    </button>
  {:else}
    <div class="flex flex-col gap-3">
      <input
        type="text"
        placeholder="Doctor's name"
        bind:value={newName}
        class="w-full px-4 py-3 rounded-xl border border-stone-200 text-stone-700
               focus:outline-none focus:border-[var(--color-primary)] min-h-[44px]"
      />
      <select
        bind:value={newSpecialty}
        class="w-full px-4 py-3 rounded-xl border border-stone-200 text-stone-700 bg-white
               focus:outline-none focus:border-[var(--color-primary)] min-h-[44px]"
      >
        {#each SPECIALTIES as spec}
          <option value={spec}>{spec}</option>
        {/each}
      </select>
      <input
        type="text"
        placeholder="Institution (optional)"
        bind:value={newInstitution}
        class="w-full px-4 py-3 rounded-xl border border-stone-200 text-stone-700
               focus:outline-none focus:border-[var(--color-primary)] min-h-[44px]"
      />
      <div class="flex gap-2">
        <button
          class="flex-1 px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
                 font-medium min-h-[44px] disabled:opacity-50"
          disabled={!newName.trim()}
          onclick={handleCreate}
        >
          Continue
        </button>
        <button
          class="px-4 py-3 text-stone-500 rounded-xl min-h-[44px]"
          onclick={() => showNewForm = false}
        >
          Cancel
        </button>
      </div>
    </div>
  {/if}
</div>
