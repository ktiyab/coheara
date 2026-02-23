<!-- D6 + F6: Desktop sidebar — flat list, 3px left bar, MD icons, profile popover. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { profile } from '$lib/stores/profile.svelte';
  import { profiles } from '$lib/stores/profiles.svelte';
  import { lockProfile } from '$lib/api/profile';
  import { dispatchProfileSwitch } from '$lib/utils/session-events';
  import { PROFILE_COLORS, type ProfileInfo } from '$lib/types/profile';
  import { HomeIcon, SearchIcon, HistoryIcon, DocsIcon, TimelineIcon, SettingsIcon, ChevronLeftIcon, ChevronRightIcon, ChevronDownIcon } from '$lib/components/icons/md';
  import ProfilePopover from '$lib/components/profile/ProfilePopover.svelte';
  import { extraction } from '$lib/stores/extraction.svelte';
  import type { Component } from 'svelte';

  type NavItem = {
    id: string;
    key: string;
    Icon: Component<{ class?: string }>;
  };

  const navItems: NavItem[] = [
    { id: 'home', key: 'nav.home', Icon: HomeIcon },
    { id: 'chat', key: 'nav.chat', Icon: SearchIcon },
    { id: 'history', key: 'nav.history', Icon: HistoryIcon },
    { id: 'documents', key: 'nav.documents', Icon: DocsIcon },
    { id: 'timeline', key: 'nav.timeline', Icon: TimelineIcon },
    { id: 'settings', key: 'nav.settings', Icon: SettingsIcon },
  ];

  let collapsed = $derived(navigation.sidebarCollapsed);
  let popoverOpen = $state(false);

  /** F6: Derive managed profiles for the active user. */
  let activeInfo = $derived(profile.activeInfo);
  let managedProfiles = $derived(
    activeInfo ? profiles.managedBy(activeInfo.name) : []
  );
  let profileColor = $derived(
    activeInfo?.color_index != null
      ? PROFILE_COLORS[activeInfo.color_index % PROFILE_COLORS.length]
      : null
  );

  function handleNav(screen: string) {
    navigation.navigate(screen);
  }

  /** F7: Switch to a specific profile — lock + dispatch event with target ID. */
  async function handleSwitchTo(targetProfile: ProfileInfo) {
    popoverOpen = false;
    await lockProfile();
    dispatchProfileSwitch(targetProfile.id);
  }

  function handleManage() {
    popoverOpen = false;
    navigation.navigate('profiles');
  }

  function handleAdd() {
    popoverOpen = false;
    navigation.navigate('profiles-create');
  }

  /** F7: Lock without targeting a specific profile — picker shows all. */
  async function handleLock() {
    popoverOpen = false;
    await lockProfile();
    dispatchProfileSwitch();
  }
</script>

<nav
  class="flex flex-col h-full border-r border-stone-200 dark:border-gray-700
         bg-white dark:bg-gray-900 transition-[width] duration-200 overflow-hidden
         {collapsed ? 'w-[var(--sidebar-collapsed-width)]' : 'w-[var(--sidebar-width)]'}"
  aria-label={$t('nav.sidebar') ?? 'Sidebar navigation'}
>
  <!-- Brand + collapse toggle -->
  <div class="flex items-center h-14 px-3 border-b border-stone-100 dark:border-gray-800 flex-shrink-0">
    {#if !collapsed}
      <span class="text-base font-bold text-[var(--color-primary)] tracking-tight flex-1 ml-1">Coheara</span>
    {/if}
    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center rounded-lg
             text-stone-400 dark:text-gray-500 hover:bg-stone-100 dark:hover:bg-gray-800 transition-colors
             {collapsed ? 'mx-auto' : ''}"
      onclick={() => navigation.toggleSidebar()}
      aria-label={collapsed ? ($t('nav.expand_sidebar') ?? 'Expand sidebar') : ($t('nav.collapse_sidebar') ?? 'Collapse sidebar')}
      title={collapsed ? ($t('nav.expand_sidebar') ?? 'Expand sidebar') : ($t('nav.collapse_sidebar') ?? 'Collapse sidebar')}
    >
      {#if collapsed}
        <ChevronRightIcon class="w-5 h-5" />
      {:else}
        <ChevronLeftIcon class="w-5 h-5" />
      {/if}
    </button>
  </div>

  <!-- Navigation — flat list, no section labels (AUDIT_01 §1B) -->
  <div class="flex-1 overflow-y-auto py-2">
    <ul class="space-y-0.5">
      {#each navItems as item}
        {@const isActive = navigation.activeScreen === item.id}
        <li>
          <button
            class="w-full flex items-center gap-3 transition-colors
                   min-h-[44px] border-l-[3px]
                   {collapsed ? 'justify-center px-2' : 'px-4'}
                   {isActive
                     ? 'border-[var(--color-interactive)] text-[var(--color-interactive)] font-medium'
                     : 'border-transparent text-stone-600 dark:text-gray-400 hover:bg-stone-50 dark:hover:bg-gray-800/50'}"
            onclick={() => handleNav(item.id)}
            aria-current={isActive ? 'page' : undefined}
            title={collapsed ? ($t(item.key) ?? item.id) : undefined}
          >
            <span class="flex-shrink-0 relative">
              <item.Icon class="w-6 h-6" />
              {#if item.id === 'home' && extraction.count > 0 && collapsed}
                <span class="absolute -top-1 -right-1 w-2.5 h-2.5 rounded-full bg-[var(--color-primary)]"></span>
              {/if}
            </span>
            {#if !collapsed}
              <span class="text-sm truncate flex-1">{$t(item.key)}</span>
              {#if item.id === 'home' && extraction.count > 0}
                <span class="text-[10px] font-bold px-1.5 py-0.5 rounded-full bg-[var(--color-primary)] text-white flex-shrink-0">
                  {extraction.count}
                </span>
              {/if}
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  </div>

  <!-- F6: Profile section (bottom) — popover trigger -->
  <div class="flex-shrink-0 border-t border-stone-100 dark:border-gray-800 p-2 relative">
    {#if collapsed}
      <!-- Collapsed: colored avatar circle — click opens popover -->
      <button
        class="w-full flex items-center justify-center rounded-lg min-h-[44px]
               hover:bg-stone-100 dark:hover:bg-gray-800 transition-colors"
        onclick={() => { popoverOpen = !popoverOpen; }}
        title={profile.name ?? 'Profile'}
        aria-label={$t('profile.profiles_heading') ?? 'Profiles'}
        aria-expanded={popoverOpen}
      >
        <div
          class="w-8 h-8 rounded-full flex items-center justify-center text-white text-xs font-bold flex-shrink-0"
          style:background-color={profileColor ?? 'var(--color-primary)'}
        >
          {(profile.name ?? 'P').charAt(0).toUpperCase()}
        </div>
      </button>
    {:else}
      <!-- Expanded: avatar + name + chevron — click toggles popover -->
      <button
        class="w-full flex items-center gap-2.5 rounded-lg px-3 py-2 min-h-[44px]
               hover:bg-stone-50 dark:hover:bg-gray-800/50 transition-colors text-left"
        onclick={() => { popoverOpen = !popoverOpen; }}
        aria-label={$t('profile.profiles_heading') ?? 'Profiles'}
        aria-expanded={popoverOpen}
      >
        <div
          class="w-8 h-8 rounded-full flex items-center justify-center text-white text-xs font-bold flex-shrink-0"
          style:background-color={profileColor ?? 'var(--color-primary)'}
        >
          {(profile.name ?? 'P').charAt(0).toUpperCase()}
        </div>
        <div class="flex-1 min-w-0">
          <p class="text-sm text-stone-700 dark:text-gray-300 truncate font-medium">{profile.name ?? 'Patient'}</p>
          <p class="text-xs text-stone-400 dark:text-gray-500 truncate">{$t('nav.profile_status_active') ?? 'Active profile'}</p>
        </div>
        <ChevronDownIcon class="w-4 h-4 text-stone-400 dark:text-gray-500 flex-shrink-0 transition-transform {popoverOpen ? 'rotate-180' : ''}" />
      </button>
    {/if}

    <!-- Popover -->
    {#if popoverOpen && activeInfo}
      <ProfilePopover
        activeProfile={activeInfo}
        {managedProfiles}
        isSelfManaged={profile.isSelfManaged}
        onSwitchTo={handleSwitchTo}
        onManage={handleManage}
        onAdd={handleAdd}
        onLock={handleLock}
        onClose={() => { popoverOpen = false; }}
      />
    {/if}
  </div>
</nav>
