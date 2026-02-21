<!-- L4-02: Professional selector — choose existing or create new. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { ProfessionalInfo } from '$lib/types/appointment';
  import { SPECIALTIES } from '$lib/types/appointment';
  import Button from '$lib/components/ui/Button.svelte';

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
        class="w-full text-left p-4 bg-white dark:bg-gray-900 rounded-xl border border-stone-100 dark:border-gray-800 shadow-sm
               hover:border-[var(--color-primary)] transition-colors min-h-[44px]"
        onclick={() => onSelect(prof.id)}
      >
        <p class="font-medium text-stone-800 dark:text-gray-100">{prof.name}</p>
        <p class="text-sm text-stone-500 dark:text-gray-400">
          {prof.specialty ?? $t('appointment.professional_default_specialty')}
          {#if prof.last_seen_date}
            <span class="text-stone-500 dark:text-gray-400"> · {$t('appointment.professional_last_visit')} {prof.last_seen_date}</span>
          {/if}
        </p>
        {#if prof.institution}
          <p class="text-xs text-stone-500 dark:text-gray-400 mt-0.5">{prof.institution}</p>
        {/if}
      </button>
    {/each}

    {#if professionals.length === 0}
      <p class="text-center text-stone-500 dark:text-gray-400 py-4">{$t('appointment.professional_empty')}</p>
    {/if}

    <Button variant="dashed" fullWidth onclick={() => showNewForm = true}>
      {$t('appointment.professional_add_new')}
    </Button>
  {:else}
    <div class="flex flex-col gap-3">
      <input
        type="text"
        placeholder={$t('appointment.professional_name_placeholder')}
        aria-label={$t('appointment.professional_name_placeholder')}
        bind:value={newName}
        class="w-full px-4 py-3 rounded-xl border border-stone-200 dark:border-gray-700 text-stone-700 dark:text-gray-200
               bg-white dark:bg-gray-900 focus:outline-none focus:border-[var(--color-primary)] min-h-[44px]"
      />
      <select
        bind:value={newSpecialty}
        aria-label={$t('appointment.specialty_select')}
        class="w-full px-4 py-3 rounded-xl border border-stone-200 dark:border-gray-700 text-stone-700 dark:text-gray-200 bg-white dark:bg-gray-900
               focus:outline-none focus:border-[var(--color-primary)] min-h-[44px]"
      >
        {#each SPECIALTIES as spec}
          <option value={spec}>{$t(`appointment.specialty_${spec.toLowerCase()}`)}</option>
        {/each}
      </select>
      <input
        type="text"
        placeholder={$t('appointment.professional_institution_placeholder')}
        aria-label={$t('appointment.professional_institution_placeholder')}
        bind:value={newInstitution}
        class="w-full px-4 py-3 rounded-xl border border-stone-200 dark:border-gray-700 text-stone-700 dark:text-gray-200
               bg-white dark:bg-gray-900 focus:outline-none focus:border-[var(--color-primary)] min-h-[44px]"
      />
      <div class="flex gap-2">
        <Button variant="primary" disabled={!newName.trim()} onclick={handleCreate}>
          {$t('common.continue')}
        </Button>
        <Button variant="ghost" onclick={() => showNewForm = false}>
          {$t('common.cancel')}
        </Button>
      </div>
    </div>
  {/if}
</div>
