<script lang="ts">
  import { t } from 'svelte-i18n';
  import { PROFILE_COLORS, type ProfileInfo } from '$lib/types/profile';
  import Button from '$lib/components/ui/Button.svelte';
  import Avatar from '$lib/components/ui/Avatar.svelte';

  function getColor(profile: ProfileInfo): string {
    return profile.color_index != null
      ? PROFILE_COLORS[profile.color_index % PROFILE_COLORS.length]
      : PROFILE_COLORS[0];
  }

  interface Props {
    profiles: ProfileInfo[];
    onSelect: (profile: ProfileInfo) => void;
    onCreateNew: () => void;
  }
  let { profiles, onSelect, onCreateNew }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-8 max-w-md mx-auto">
  <h2 class="text-2xl font-bold text-stone-800 dark:text-gray-100">{$t('profile.picker_heading')}</h2>

  <div class="flex flex-col gap-3 w-full">
    {#each profiles as profile}
      <button
        class="w-full flex items-center gap-4 p-4 bg-white dark:bg-gray-900 rounded-xl border border-stone-200 dark:border-gray-700
               hover:border-[var(--color-primary)] hover:shadow-sm transition-all
               min-h-[44px] text-left"
        style:border-left="3px solid {getColor(profile)}"
        onclick={() => onSelect(profile)}
      >
        <Avatar name={profile.name} variant="user" size="md" color={getColor(profile)} />
        <div class="flex flex-col">
          <span class="text-stone-800 dark:text-gray-100 font-medium text-lg">{profile.name}</span>
          {#if profile.managed_by}
            <span class="text-stone-500 dark:text-gray-400 text-sm">{$t('profile.managed_by_label', { values: { managedBy: profile.managed_by } })}</span>
          {/if}
        </div>
      </button>
    {/each}
  </div>

  <Button variant="dashed" onclick={onCreateNew}>
    {$t('profile.create_new')}
  </Button>
</div>
