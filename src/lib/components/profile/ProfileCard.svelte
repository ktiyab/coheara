<!-- F6: Profile card — reusable card for one profile in the management screen. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { PROFILE_COLORS, type ProfileInfo } from '$lib/types/profile';
  import Avatar from '$lib/components/ui/Avatar.svelte';
  import { ChevronRightIcon, DeleteIcon } from '$lib/components/icons/md';

  interface Props {
    profile: ProfileInfo;
    isActive: boolean;
    isOwnProfile: boolean;
    /** F7: Whether this profile can be deleted. Managed profiles viewing
     *  their own card should NOT see delete — only the caregiver can delete. */
    canDelete?: boolean;
    hasDependents: boolean;
    onSwitchTo: () => void;
    onDelete: () => void;
  }

  let {
    profile: profileInfo,
    isActive,
    isOwnProfile,
    canDelete = true,
    hasDependents,
    onSwitchTo,
    onDelete,
  }: Props = $props();

  let color = $derived(
    profileInfo.color_index != null
      ? PROFILE_COLORS[profileInfo.color_index % PROFILE_COLORS.length]
      : null
  );

  let createdDate = $derived(() => {
    try {
      return new Date(profileInfo.created_at).toLocaleDateString(undefined, {
        year: 'numeric',
        month: 'short',
      });
    } catch {
      return '';
    }
  });
</script>

<div
  class="flex items-center gap-4 rounded-xl px-4 py-3 border border-stone-200 dark:border-gray-700
         bg-white dark:bg-gray-800/50
         {isActive ? 'ring-2 ring-[var(--color-interactive)]/30' : ''}"
  style:border-left-width="3px"
  style:border-left-color={color ?? 'var(--color-primary)'}
>
  <Avatar name={profileInfo.name} size="lg" {color} />

  <div class="flex-1 min-w-0">
    <div class="flex items-center gap-2">
      <p class="text-sm font-medium text-stone-800 dark:text-gray-100 truncate">
        {profileInfo.name}
      </p>
      {#if isActive}
        <span class="text-xs px-1.5 py-0.5 rounded-full bg-[var(--color-interactive)]/10 text-[var(--color-interactive)] font-medium flex-shrink-0">
          {$t('nav.profile_status_active')}
        </span>
      {/if}
    </div>
    <p class="text-xs text-stone-400 dark:text-gray-500 truncate mt-0.5">
      {#if profileInfo.managed_by}
        {$t('profile.viewing_managed', { values: { caregiver: profileInfo.managed_by } })}
      {:else}
        {$t('profile.self_managed_label')}
      {/if}
      {#if createdDate()}
        <span class="mx-1">·</span>
        {$t('profile.created_label', { values: { date: createdDate() } })}
      {/if}
    </p>
  </div>

  <!-- Actions -->
  <div class="flex items-center gap-1 flex-shrink-0">
    {#if !isActive}
      <button
        class="min-h-[36px] min-w-[36px] flex items-center justify-center rounded-lg
               text-[var(--color-interactive)] hover:bg-[var(--color-interactive)]/10 transition-colors"
        onclick={onSwitchTo}
        title={$t('profile.switch_to', { values: { name: profileInfo.name } })}
        aria-label={$t('profile.switch_to', { values: { name: profileInfo.name } })}
      >
        <ChevronRightIcon class="w-10 h-10 text-[var(--color-success)]" />
      </button>
    {/if}

    {#if canDelete && !hasDependents}
      <button
        class="min-h-[36px] min-w-[36px] flex items-center justify-center rounded-lg
               text-stone-400 dark:text-gray-500 hover:bg-red-50 hover:text-red-600
               dark:hover:bg-red-900/20 dark:hover:text-red-400 transition-colors"
        onclick={onDelete}
        title={$t('settings.delete_profile') ?? 'Delete profile'}
        aria-label={$t('settings.delete_profile') ?? 'Delete profile'}
      >
        <DeleteIcon class="w-5 h-5" />
      </button>
    {:else if canDelete && hasDependents}
      <button
        class="min-h-[36px] min-w-[36px] flex items-center justify-center rounded-lg
               text-stone-300 dark:text-gray-600 cursor-not-allowed"
        disabled
        title={$t('profile.delete_has_dependents', { values: { count: '?' } })}
        aria-label={$t('profile.delete_has_dependents', { values: { count: '?' } })}
      >
        <DeleteIcon class="w-5 h-5" />
      </button>
    {/if}
  </div>
</div>
