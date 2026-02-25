<!-- D6 + F6: Desktop sidebar — flat list, 3px left bar, MD icons, profile popover. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation, PARENT_SCREEN } from '$lib/stores/navigation.svelte';
  import { profile } from '$lib/stores/profile.svelte';
  import { profiles } from '$lib/stores/profiles.svelte';
  import { lockProfile } from '$lib/api/profile';
  import { dispatchProfileSwitch } from '$lib/utils/session-events';
  import { PROFILE_COLORS, type ProfileInfo } from '$lib/types/profile';
  import { HomeIcon, SearchIcon, HistoryIcon, DocsIcon, TimelineIcon, DevicesIcon, SettingsIcon, ChevronLeftIcon, ChevronRightIcon, ChevronDownIcon } from '$lib/components/icons/md';
  import ProfilePopover from '$lib/components/profile/ProfilePopover.svelte';
  import { extraction } from '$lib/stores/extraction.svelte';
  import { theme } from '$lib/stores/theme.svelte';
  import { NAV_HUES, colorfulStyle } from '$lib/theme/colorful-mappings';
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
    { id: 'companion', key: 'nav.companion', Icon: DevicesIcon },
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
  <div class="flex items-center h-14 px-3 flex-shrink-0">
    {#if !collapsed}
      <svg class="w-7 h-7 text-[var(--color-success)] flex-shrink-0 ml-1" viewBox="100 50 600 540" fill="none" stroke="currentColor" stroke-width="12" xmlns="http://www.w3.org/2000/svg">
        <path d="M360.3,108.3c12.2-15.6,24.2-30.9,36.5-46.7c53.6,63.9,98.9,131.5,124.6,211.5c23.9-15.1,48.9-27.1,74.8-37.1c26-10,52.5-18,81.1-22.5c1.8,13.5,4.2,26.4,5.3,39.4c4.6,53.9-.2,106.9-18.8,158.1c-18.2,50.1-47.1,92.5-91.2,123.5c-28,19.7-59.2,31.9-93.1,36.4c-15.1,2-30.5,3.6-45.8,3.1c-10.9-.3-21.7-4.4-32.6-6.7c-2.2-.5-4.8-1.2-6.7-.5c-24.8,10.1-50.3,8.2-75.9,4.9c-33.8-4.4-65-15.7-93.4-34.5c-20.7-13.7-38.4-30.8-53.7-50.4c-36.5-46.5-54-100.2-59.9-158.1c-3.8-37.3-2.7-74.5,5-111.4c.2-1.1.6-2.2,1-3.9c55.4,10,106.6,30.3,154.9,59.7c19.2-60.5,50.3-114.1,87.7-164.8z"/>
        <path d="M382.7,191.7c4-3.5,7.8-6.8,11.5-10.2c1.9-1.8,3.3-2,5.5-.2c27.7,23.3,48.4,51.1,56.6,87.2c2.9,12.5,2,21.7-8.5,31.6c-21.3,20-35,45.9-47.8,71.9c-.8,1.6-1.9,3.1-3.4,5.4c-3.8-7.4-7-13.8-10.3-20.1c-12.8-24.3-27.7-47-48-65.9c-2.3-2.1-3-4.3-2.7-7.4c2.6-37.4,21.1-66.6,47.2-92.4z"/>
      </svg>
      <span class="text-base font-bold text-[var(--color-success)] tracking-tight flex-1 ml-2">Coheara</span>
    {:else}
      <svg class="w-7 h-7 text-[var(--color-success)] flex-shrink-0 mx-auto" viewBox="100 50 600 540" fill="none" stroke="currentColor" stroke-width="12" xmlns="http://www.w3.org/2000/svg">
        <path d="M360.3,108.3c12.2-15.6,24.2-30.9,36.5-46.7c53.6,63.9,98.9,131.5,124.6,211.5c23.9-15.1,48.9-27.1,74.8-37.1c26-10,52.5-18,81.1-22.5c1.8,13.5,4.2,26.4,5.3,39.4c4.6,53.9-.2,106.9-18.8,158.1c-18.2,50.1-47.1,92.5-91.2,123.5c-28,19.7-59.2,31.9-93.1,36.4c-15.1,2-30.5,3.6-45.8,3.1c-10.9-.3-21.7-4.4-32.6-6.7c-2.2-.5-4.8-1.2-6.7-.5c-24.8,10.1-50.3,8.2-75.9,4.9c-33.8-4.4-65-15.7-93.4-34.5c-20.7-13.7-38.4-30.8-53.7-50.4c-36.5-46.5-54-100.2-59.9-158.1c-3.8-37.3-2.7-74.5,5-111.4c.2-1.1.6-2.2,1-3.9c55.4,10,106.6,30.3,154.9,59.7c19.2-60.5,50.3-114.1,87.7-164.8z"/>
        <path d="M382.7,191.7c4-3.5,7.8-6.8,11.5-10.2c1.9-1.8,3.3-2,5.5-.2c27.7,23.3,48.4,51.1,56.6,87.2c2.9,12.5,2,21.7-8.5,31.6c-21.3,20-35,45.9-47.8,71.9c-.8,1.6-1.9,3.1-3.4,5.4c-3.8-7.4-7-13.8-10.3-20.1c-12.8-24.3-27.7-47-48-65.9c-2.3-2.1-3-4.3-2.7-7.4c2.6-37.4,21.1-66.6,47.2-92.4z"/>
      </svg>
    {/if}
    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center rounded-lg
             text-[var(--color-success)] hover:bg-stone-100 dark:hover:bg-gray-800 transition-colors
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
      {#each navItems as item, i}
        {@const isActive = navigation.activeScreen === item.id || PARENT_SCREEN[navigation.activeScreen] === item.id}
        <li>
          <button
            style={theme.isColorful ? colorfulStyle(NAV_HUES[i]) : undefined}
            class="w-full flex items-center gap-3 transition-colors
                   min-h-[44px] border-l-[3px]
                   {collapsed ? 'justify-center px-2' : 'px-4'}
                   {isActive
                     ? 'border-[var(--color-success)] text-[var(--color-success)] font-medium'
                     : theme.isColorful
                       ? 'border-transparent text-[var(--color-success-800)] hover:bg-[var(--color-success-50)]'
                       : 'border-transparent text-stone-600 dark:text-gray-400 hover:bg-stone-50 dark:hover:bg-gray-800/50'}"
            onclick={() => handleNav(item.id)}
            aria-current={isActive ? 'page' : undefined}
            title={collapsed ? ($t(item.key) ?? item.id) : undefined}
          >
            <span class="flex-shrink-0 relative">
              <item.Icon class="w-8 h-8" />
              {#if item.id === 'home' && extraction.count > 0 && collapsed}
                <span class="absolute -top-1 -right-1 w-2.5 h-2.5 rounded-full bg-[var(--color-primary)]"></span>
              {/if}
            </span>
            {#if !collapsed}
              <span class="text-base truncate">{$t(item.key)}</span>
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
  <div class="flex-shrink-0 p-2 relative">
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
        {collapsed}
        onSwitchTo={handleSwitchTo}
        onManage={handleManage}
        onAdd={handleAdd}
        onLock={handleLock}
        onClose={() => { popoverOpen = false; }}
      />
    {/if}
  </div>
</nav>
