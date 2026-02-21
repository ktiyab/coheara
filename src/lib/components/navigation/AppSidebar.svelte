<!-- D6: Desktop sidebar — replaces TabBar. All 9 screens visible, grouped by section. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation, NAV_SECTIONS } from '$lib/stores/navigation.svelte';
  import { profile } from '$lib/stores/profile.svelte';
  import { extraction } from '$lib/stores/extraction.svelte';
  import { lockProfile } from '$lib/api/profile';
  import {
    HomeSolid, HomeOutline,
    MessagesSolid, MessagesOutline,
    HeartSolid, HeartOutline,
    FileSolid, FileOutline,
    ClockSolid, ClockOutline,
    ClipboardSolid, ClipboardOutline,
    CogSolid, CogOutline,
    ChevronDoubleLeftOutline, ChevronDoubleRightOutline,
    LockSolid, UserSolid
  } from 'flowbite-svelte-icons';
  import { PillIcon } from '$lib/components/icons';

  import type { Component } from 'svelte';

  type NavItem = {
    id: string;
    key: string;
    Active: Component<{ class?: string }>;
    Inactive: Component<{ class?: string }>;
  };

  const navItems: Record<string, NavItem[]> = {
    main: [
      { id: 'home', key: 'nav.home', Active: HomeSolid, Inactive: HomeOutline },
      { id: 'chat', key: 'nav.chat', Active: MessagesSolid, Inactive: MessagesOutline },
      { id: 'journal', key: 'nav.journal', Active: HeartSolid, Inactive: HeartOutline },
      { id: 'medications', key: 'nav.medications', Active: PillIcon, Inactive: PillIcon },
    ],
    library: [
      { id: 'documents', key: 'nav.documents', Active: FileSolid, Inactive: FileOutline },
      { id: 'timeline', key: 'nav.timeline', Active: ClockSolid, Inactive: ClockOutline },
      { id: 'appointments', key: 'nav.appointments', Active: ClipboardSolid, Inactive: ClipboardOutline },
    ],
    system: [
      { id: 'settings', key: 'nav.settings', Active: CogSolid, Inactive: CogOutline },
    ],
  };

  const sectionKeys: (keyof typeof NAV_SECTIONS)[] = ['main', 'library', 'system'];

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
        <ChevronDoubleRightOutline class="w-4 h-4" />
      {:else}
        <ChevronDoubleLeftOutline class="w-4 h-4" />
      {/if}
    </button>
  </div>

  <!-- Navigation sections -->
  <div class="flex-1 overflow-y-auto py-2 px-2">
    {#each sectionKeys as section, sectionIdx}
      {#if sectionIdx > 0}
        <div class="border-t border-stone-100 dark:border-gray-800 my-2"></div>
      {/if}

      {#if !collapsed}
        <p class="px-2 pt-2 pb-1 text-[10px] font-semibold uppercase tracking-wider text-stone-400 dark:text-gray-500">
          {$t(`nav.section_${section}`) ?? section}
        </p>
      {/if}

      <ul class="space-y-0.5">
        {#each navItems[section] as item}
          {@const isActive = navigation.activeScreen === item.id}
          <li>
            <button
              class="w-full flex items-center gap-3 rounded-lg transition-colors
                     min-h-[40px] {collapsed ? 'justify-center px-2' : 'px-3'}
                     {isActive
                       ? 'bg-[var(--color-interactive)]/10 text-[var(--color-interactive)] font-medium'
                       : 'text-stone-600 dark:text-gray-400 hover:bg-stone-100 dark:hover:bg-gray-800'}"
              onclick={() => handleNav(item.id)}
              aria-current={isActive ? 'page' : undefined}
              title={collapsed ? ($t(item.key) ?? item.id) : undefined}
            >
              <span class="relative flex-shrink-0">
                {#if isActive}
                  <item.Active class="w-5 h-5" />
                {:else}
                  <item.Inactive class="w-5 h-5" />
                {/if}
                {#if collapsed && item.id === 'home' && extraction.count > 0}
                  <span class="absolute -top-1 -right-1 w-2.5 h-2.5 rounded-full bg-[var(--color-primary)] border-2 border-white dark:border-gray-900"></span>
                {/if}
              </span>
              {#if !collapsed}
                <span class="text-sm truncate flex-1">{$t(item.key)}</span>
                {#if item.id === 'home' && extraction.count > 0}
                  <span
                    class="ml-auto text-[10px] font-bold min-w-[18px] h-[18px] px-1 rounded-full flex items-center justify-center
                           bg-[var(--color-primary)] text-white"
                    aria-label="{extraction.count} pending"
                  >
                    {extraction.count > 99 ? '99+' : extraction.count}
                  </span>
                {/if}
              {/if}
            </button>
          </li>
        {/each}
      </ul>
    {/each}
  </div>

  <!-- Profile section (bottom) -->
  <div class="flex-shrink-0 border-t border-stone-100 dark:border-gray-800 p-2">
    {#if collapsed}
      <button
        class="w-full flex items-center justify-center rounded-lg min-h-[40px]
               text-stone-500 dark:text-gray-400 hover:bg-stone-100 dark:hover:bg-gray-800 transition-colors"
        onclick={async () => { await lockProfile(); }}
        title={$t('settings.hub_switch_title') ?? 'Lock & switch profile'}
      >
        <LockSolid class="w-5 h-5" />
      </button>
    {:else}
      <div class="flex items-center gap-2 px-2 py-1.5">
        <div class="w-7 h-7 rounded-full bg-[var(--color-primary)] flex items-center justify-center text-white text-xs font-bold flex-shrink-0">
          {(profile.name ?? 'P').charAt(0).toUpperCase()}
        </div>
        <span class="text-sm text-stone-700 dark:text-gray-300 truncate flex-1">{profile.name ?? 'Patient'}</span>
        <button
          class="min-h-[36px] min-w-[36px] flex items-center justify-center rounded-lg
                 text-stone-400 dark:text-gray-500 hover:bg-stone-100 dark:hover:bg-gray-800 transition-colors"
          onclick={async () => { await lockProfile(); }}
          title={$t('settings.hub_switch_title') ?? 'Lock & switch profile'}
          aria-label={$t('settings.hub_switch_title') ?? 'Lock & switch profile'}
        >
          <LockSolid class="w-4 h-4" />
        </button>
      </div>
    {/if}
  </div>
</nav>
