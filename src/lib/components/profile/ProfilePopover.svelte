<!-- F6: Profile popover — Apple/Netflix-style flyout from sidebar bottom.
     Shows active profile, family members, add/manage/lock actions. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { PROFILE_COLORS, type ProfileInfo } from '$lib/types/profile';
  import Avatar from '$lib/components/ui/Avatar.svelte';
  import { GroupIcon, PlusIcon, SettingsIcon, LockIcon, ChevronRightIcon } from '$lib/components/icons/md';

  interface Props {
    activeProfile: ProfileInfo;
    managedProfiles: ProfileInfo[];
    isSelfManaged: boolean;
    collapsed?: boolean;
    onSwitchTo: (profile: ProfileInfo) => void;
    onManage: () => void;
    onAdd: () => void;
    onLock: () => void;
    onClose: () => void;
  }

  let {
    activeProfile,
    managedProfiles,
    isSelfManaged,
    collapsed = false,
    onSwitchTo,
    onManage,
    onAdd,
    onLock,
    onClose,
  }: Props = $props();

  function profileColor(p: ProfileInfo): string | null {
    return p.color_index != null
      ? PROFILE_COLORS[p.color_index % PROFILE_COLORS.length]
      : null;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      onClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Backdrop — click to close -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-40"
  onclick={onClose}
  onkeydown={() => {}}
></div>

<!-- Popover card -->
<div
  class="{collapsed
    ? 'fixed bottom-14 left-[var(--sidebar-collapsed-width)]'
    : 'absolute bottom-full left-0'} mb-2 w-[var(--sidebar-width)] z-50
         bg-white dark:bg-gray-800 rounded-xl shadow-lg border border-stone-200 dark:border-gray-700
         overflow-hidden"
  role="dialog"
  aria-label={$t('profile.profiles_heading') ?? 'Profiles'}
>
  <!-- Active profile section -->
  <div class="px-4 py-3 border-b border-stone-100 dark:border-gray-700">
    <div class="flex items-center gap-3">
      <Avatar name={activeProfile.name} size="md" color={profileColor(activeProfile)} />
      <div class="flex-1 min-w-0">
        <p class="text-sm font-medium text-stone-800 dark:text-gray-100 truncate">
          {activeProfile.name}
        </p>
        <p class="text-xs text-stone-400 dark:text-gray-500 truncate">
          {activeProfile.managed_by
            ? $t('profile.viewing_managed', { values: { caregiver: activeProfile.managed_by } })
            : $t('profile.self_managed_label')}
        </p>
      </div>
      <button
        class="min-h-[36px] min-w-[36px] flex items-center justify-center rounded-lg
               text-stone-400 dark:text-gray-500 hover:bg-stone-100 dark:hover:bg-gray-700 transition-colors"
        onclick={onLock}
        title={$t('profile.lock') ?? 'Lock'}
        aria-label={$t('profile.lock') ?? 'Lock'}
      >
        <LockIcon class="w-5 h-5" />
      </button>
    </div>
  </div>

  <!-- Family members section -->
  {#if managedProfiles.length > 0}
    <div class="px-4 py-2 border-b border-stone-100 dark:border-gray-700">
      <p class="text-xs font-medium text-stone-400 dark:text-gray-500 uppercase tracking-wider mb-1">
        {$t('profile.popover_family_heading')}
      </p>
      <ul class="space-y-0.5">
        {#each managedProfiles as mp}
          <li>
            <button
              class="w-full flex items-center gap-3 rounded-lg px-2 py-2 min-h-[44px]
                     text-stone-700 dark:text-gray-300 hover:bg-stone-50 dark:hover:bg-gray-700/50 transition-colors"
              onclick={() => onSwitchTo(mp)}
              aria-label={$t('profile.switch_to', { values: { name: mp.name } })}
            >
              <Avatar name={mp.name} size="sm" color={profileColor(mp)} />
              <span class="flex-1 text-sm text-left truncate">{mp.name}</span>
              <ChevronRightIcon class="w-8 h-8 text-[var(--color-success)] flex-shrink-0" />
            </button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}

  <!-- Actions section -->
  <div class="px-2 py-2 space-y-0.5">
    {#if isSelfManaged}
      <button
        class="w-full flex items-center gap-3 rounded-lg px-3 py-2 min-h-[44px]
               text-stone-600 dark:text-gray-400 hover:bg-stone-50 dark:hover:bg-gray-700/50 transition-colors"
        onclick={onAdd}
      >
        <PlusIcon class="w-5 h-5 text-stone-400 dark:text-gray-500" />
        <span class="text-sm">{$t('profile.popover_add')}</span>
      </button>
    {/if}
    <button
      class="w-full flex items-center gap-3 rounded-lg px-3 py-2 min-h-[44px]
             text-stone-600 dark:text-gray-400 hover:bg-stone-50 dark:hover:bg-gray-700/50 transition-colors"
      onclick={onManage}
    >
      <SettingsIcon class="w-5 h-5 text-stone-400 dark:text-gray-500" />
      <span class="text-sm">{$t('profile.popover_manage')}</span>
    </button>
  </div>
</div>
