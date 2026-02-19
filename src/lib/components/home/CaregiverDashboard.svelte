<!-- Spec 46 [CG-02]: Caregiver home dashboard showing dependent overview cards -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { CaregiverSummary } from '$lib/api/profile';
  import { lockProfile } from '$lib/api/profile';
  import { navigation } from '$lib/stores/navigation.svelte';
  import DependentCard from './DependentCard.svelte';

  interface Props {
    dependents: CaregiverSummary[];
  }
  let { dependents }: Props = $props();

  async function viewDependentProfile() {
    // Lock current profile → navigate to picker for re-authentication
    await lockProfile();
    navigation.navigate('home');
  }
</script>

{#if dependents.length > 0}
  <div class="px-6 py-4">
    <h2 class="text-sm font-medium text-stone-500 uppercase tracking-wide mb-3">
      {$t('caregiver.dependents_heading')}
    </h2>
    <div class="flex flex-col gap-3">
      {#each dependents as summary (summary.managed_profile_id)}
        <DependentCard {summary} onViewProfile={viewDependentProfile} />
      {/each}
    </div>
  </div>
{/if}
