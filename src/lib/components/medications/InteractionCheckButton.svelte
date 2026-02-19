<!-- Spec 49 [FE-03]: Drug interaction check — navigates to chat with structured prompt. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import type { MedicationCard } from '$lib/types/medication';
  import Button from '$lib/components/ui/Button.svelte';

  interface Props {
    medications: MedicationCard[];
  }

  let { medications }: Props = $props();

  let activeMeds = $derived(medications.filter((m) => m.status === 'Active'));

  function checkInteractions() {
    if (activeMeds.length < 2) return;

    const medList = activeMeds
      .map((m, i) => `${i + 1}. ${m.generic_name} ${m.dose} (${m.route}, ${m.frequency})`)
      .join('\n');

    const prompt = `Check for drug interactions between my active medications:\n${medList}\n\nFor each interacting pair, tell me: is there a known interaction, what is the severity (none/mild/moderate/serious), and what should I do?`;

    navigation.navigate('chat', { prefill: prompt });
  }
</script>

{#if activeMeds.length >= 2}
  <div class="px-6 py-2">
    <Button variant="ghost" fullWidth onclick={checkInteractions}>
      {$t('medications.check_interactions')}
    </Button>
  </div>
{/if}
