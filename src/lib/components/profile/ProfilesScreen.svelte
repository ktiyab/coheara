<!-- F6: Profiles management screen — list, switch, delete, add.
     Accessible from sidebar popover "Manage profiles" or Settings row. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import { profile } from '$lib/stores/profile.svelte';
  import { profiles } from '$lib/stores/profiles.svelte';
  import { lockProfile, deleteProfile } from '$lib/api/profile';
  import { dispatchProfileSwitch } from '$lib/utils/session-events';
  import { navigation } from '$lib/stores/navigation.svelte';
  import type { ProfileInfo } from '$lib/types/profile';
  import ProfileCard from './ProfileCard.svelte';
  import { PlusIcon } from '$lib/components/icons/md';

  let activeInfo = $derived(profile.activeInfo);
  let familyProfiles = $derived(
    activeInfo ? profiles.managedBy(activeInfo.name) : []
  );

  let deleteError = $state<string | null>(null);

  onMount(() => {
    profiles.refresh();
  });

  /** F7: Switch to a specific profile — lock + dispatch event with target ID. */
  async function handleSwitchTo(targetProfile: ProfileInfo) {
    await lockProfile();
    dispatchProfileSwitch(targetProfile.id);
  }

  async function handleDelete(profileId: string) {
    deleteError = null;
    try {
      await deleteProfile(profileId);
      await profiles.refresh();
    } catch (e) {
      deleteError = e instanceof Error ? e.message : String(e);
    }
  }
</script>

<div class="max-w-2xl mx-auto px-[var(--spacing-page-x)] py-8">
  <h1 class="text-xl font-semibold text-stone-800 dark:text-gray-100 mb-6">
    {$t('profile.profiles_heading')}
  </h1>

  {#if deleteError}
    <div class="mb-4 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-sm text-red-700 dark:text-red-300">
      {deleteError}
    </div>
  {/if}

  <!-- Your profile section -->
  {#if activeInfo}
    <section class="mb-8">
      <h2 class="text-xs font-medium text-stone-400 dark:text-gray-500 uppercase tracking-wider mb-3">
        {$t('profile.your_profile_section')}
      </h2>
      <ProfileCard
        profile={activeInfo}
        isActive={true}
        isOwnProfile={true}
        canDelete={profile.isSelfManaged}
        hasDependents={profiles.hasDependents(activeInfo.name)}
        onSwitchTo={() => {}}
        onDelete={() => handleDelete(activeInfo.id)}
      />
    </section>
  {/if}

  <!-- Family members section -->
  {#if profile.isSelfManaged}
    <section class="mb-8">
      <h2 class="text-xs font-medium text-stone-400 dark:text-gray-500 uppercase tracking-wider mb-3">
        {$t('profile.family_section')}
      </h2>

      {#if familyProfiles.length > 0}
        <div class="space-y-3">
          {#each familyProfiles as fp}
            <ProfileCard
              profile={fp}
              isActive={false}
              isOwnProfile={false}
              canDelete={true}
              hasDependents={profiles.hasDependents(fp.name)}
              onSwitchTo={() => handleSwitchTo(fp)}
              onDelete={() => handleDelete(fp.id)}
            />
          {/each}
        </div>
      {:else}
        <div class="rounded-xl border border-dashed border-stone-200 dark:border-gray-700 px-6 py-8 text-center">
          <p class="text-sm text-stone-500 dark:text-gray-400">
            {$t('profile.no_family_members')}
          </p>
          <p class="text-xs text-stone-400 dark:text-gray-500 mt-1">
            {$t('profile.no_family_hint')}
          </p>
        </div>
      {/if}
    </section>

    <!-- Add button -->
    <button
      class="flex items-center gap-2 rounded-lg px-4 py-3 min-h-[44px]
             border border-dashed border-stone-300 dark:border-gray-600
             text-stone-600 dark:text-gray-400 hover:border-[var(--color-interactive)] hover:text-[var(--color-interactive)]
             dark:hover:border-[var(--color-interactive)] dark:hover:text-[var(--color-interactive)]
             transition-colors w-full justify-center"
      onclick={() => navigation.navigate('profiles-create')}
    >
      <PlusIcon class="w-5 h-5" />
      <span class="text-sm font-medium">{$t('profile.popover_add')}</span>
    </button>
  {/if}
</div>
