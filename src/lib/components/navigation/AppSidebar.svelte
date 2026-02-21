<!-- D6: Desktop sidebar — LP-06 + AUDIT_01: flat list, 3px left bar, MD icons. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { profile } from '$lib/stores/profile.svelte';
  import { lockProfile } from '$lib/api/profile';
  import { HomeIcon, SearchIcon, DocsIcon, TimelineIcon, SettingsIcon, ChevronLeftIcon, ChevronRightIcon, LockIcon } from '$lib/components/icons/md';
  import type { Component } from 'svelte';

  type NavItem = {
    id: string;
    key: string;
    Icon: Component<{ class?: string }>;
  };

  const navItems: NavItem[] = [
    { id: 'home', key: 'nav.home', Icon: HomeIcon },
    { id: 'chat', key: 'nav.chat', Icon: SearchIcon },
    { id: 'documents', key: 'nav.documents', Icon: DocsIcon },
    { id: 'timeline', key: 'nav.timeline', Icon: TimelineIcon },
    { id: 'settings', key: 'nav.settings', Icon: SettingsIcon },
  ];

  let collapsed = $derived(navigation.sidebarCollapsed);

  function handleNav(screen: string) {
    navigation.navigate(screen);
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
        <ChevronRightIcon class="w-4 h-4" />
      {:else}
        <ChevronLeftIcon class="w-4 h-4" />
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
            <span class="flex-shrink-0">
              <item.Icon class="w-5 h-5" />
            </span>
            {#if !collapsed}
              <span class="text-sm truncate flex-1">{$t(item.key)}</span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  </div>

  <!-- Profile section (bottom) — 2-line layout (AUDIT_01 §1D) -->
  <div class="flex-shrink-0 border-t border-stone-100 dark:border-gray-800 p-2">
    {#if collapsed}
      <button
        class="w-full flex items-center justify-center rounded-lg min-h-[44px]
               text-stone-500 dark:text-gray-400 hover:bg-stone-100 dark:hover:bg-gray-800 transition-colors"
        onclick={async () => { await lockProfile(); }}
        title={$t('settings.hub_switch_title') ?? 'Lock & switch profile'}
      >
        <LockIcon class="w-5 h-5" />
      </button>
    {:else}
      <div class="px-3 py-2">
        <!-- Line 1: Avatar + name -->
        <div class="flex items-center gap-2.5">
          <div class="w-8 h-8 rounded-full bg-[var(--color-primary)] flex items-center justify-center text-white text-xs font-bold flex-shrink-0">
            {(profile.name ?? 'P').charAt(0).toUpperCase()}
          </div>
          <div class="flex-1 min-w-0">
            <p class="text-sm text-stone-700 dark:text-gray-300 truncate font-medium">{profile.name ?? 'Patient'}</p>
            <p class="text-xs text-stone-400 dark:text-gray-500 truncate">{$t('nav.profile_status_active') ?? 'Active profile'}</p>
          </div>
        </div>
        <!-- Line 2: Utility icons -->
        <div class="flex items-center gap-1 mt-2">
          <button
            class="min-h-[32px] min-w-[32px] flex items-center justify-center rounded-lg
                   text-stone-400 dark:text-gray-500 hover:bg-stone-100 dark:hover:bg-gray-800 transition-colors"
            onclick={() => navigation.navigate('settings')}
            title={$t('nav.settings') ?? 'Settings'}
            aria-label={$t('nav.settings') ?? 'Settings'}
          >
            <SettingsIcon class="w-4 h-4" />
          </button>
          <button
            class="min-h-[32px] min-w-[32px] flex items-center justify-center rounded-lg
                   text-stone-400 dark:text-gray-500 hover:bg-stone-100 dark:hover:bg-gray-800 transition-colors"
            onclick={async () => { await lockProfile(); }}
            title={$t('settings.hub_switch_title') ?? 'Lock & switch profile'}
            aria-label={$t('settings.hub_switch_title') ?? 'Lock & switch profile'}
          >
            <LockIcon class="w-4 h-4" />
          </button>
        </div>
      </div>
    {/if}
  </div>
</nav>
