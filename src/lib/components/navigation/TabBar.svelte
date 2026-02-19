<script lang="ts">
  import { tick } from 'svelte';
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';

  const tabs = [
    { id: 'home', key: 'nav.home', icon: '⌂' },
    { id: 'chat', key: 'nav.chat', icon: '💬' },
    { id: 'journal', key: 'nav.journal', icon: '♡' },
    { id: 'medications', key: 'nav.medications', icon: '💊' },
    { id: 'more', key: 'nav.more', icon: '⋯' },
  ];

  let showMore = $state(false);
  let focusedIndex = $state(-1);
  let menuRef: HTMLDivElement | undefined = $state(undefined);
  let moreButtonRef: HTMLButtonElement | undefined = $state(undefined);

  const moreItems = [
    { id: 'documents', key: 'nav.documents' },
    { id: 'timeline', key: 'nav.timeline' },
    { id: 'appointments', key: 'nav.appointments' },
    { id: 'settings', key: 'nav.settings' },
  ];

  async function openMenu() {
    showMore = true;
    focusedIndex = 0;
    await tick();
    focusMenuItem(0);
  }

  function closeMenu() {
    showMore = false;
    focusedIndex = -1;
    moreButtonRef?.focus();
  }

  function focusMenuItem(index: number) {
    const items = menuRef?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]');
    items?.[index]?.focus();
  }

  function handleMenuKeydown(e: KeyboardEvent) {
    switch (e.key) {
      case 'Escape':
        e.preventDefault();
        closeMenu();
        break;
      case 'ArrowDown':
        e.preventDefault();
        focusedIndex = (focusedIndex + 1) % moreItems.length;
        focusMenuItem(focusedIndex);
        break;
      case 'ArrowUp':
        e.preventDefault();
        focusedIndex = (focusedIndex - 1 + moreItems.length) % moreItems.length;
        focusMenuItem(focusedIndex);
        break;
      case 'Home':
        e.preventDefault();
        focusedIndex = 0;
        focusMenuItem(0);
        break;
      case 'End':
        e.preventDefault();
        focusedIndex = moreItems.length - 1;
        focusMenuItem(focusedIndex);
        break;
      case 'Tab':
        closeMenu();
        break;
    }
  }
</script>

<nav
  class="fixed bottom-0 left-0 right-0 bg-white border-t border-stone-200
         flex items-center justify-around h-16 z-50"
>
  {#each tabs as tab}
    {#if tab.id === 'more'}
      <button
        bind:this={moreButtonRef}
        class="flex flex-col items-center justify-center gap-1 flex-1 h-full
               min-h-[44px] min-w-[44px]
               {navigation.activeTab === 'more' ? 'text-[var(--color-interactive)]' : 'text-stone-500'}"
        onclick={() => { showMore ? closeMenu() : openMenu(); }}
        aria-label={$t(tab.key)}
        aria-haspopup="menu"
        aria-expanded={showMore}
      >
        <span class="text-lg">{tab.icon}</span>
        <span class="text-xs">{$t(tab.key)}</span>
      </button>
    {:else}
      <button
        class="flex flex-col items-center justify-center gap-1 flex-1 h-full
               min-h-[44px] min-w-[44px]
               {navigation.activeTab === tab.id ? 'text-[var(--color-interactive)]' : 'text-stone-500'}"
        onclick={() => {
          showMore = false;
          navigation.navigate(tab.id);
        }}
        aria-current={navigation.activeTab === tab.id ? 'page' : undefined}
        aria-label={$t(tab.key)}
      >
        <span class="text-lg">{tab.icon}</span>
        <span class="text-xs">{$t(tab.key)}</span>
      </button>
    {/if}
  {/each}
</nav>

{#if showMore}
  <!-- Backdrop to close menu on outside click -->
  <button
    class="fixed inset-0 z-40 bg-transparent cursor-default"
    aria-hidden="true"
    tabindex="-1"
    onclick={closeMenu}
  ></button>
  <!-- eslint-disable-next-line svelte/no-static-element-interactions -->
  <div
    bind:this={menuRef}
    class="fixed bottom-16 right-2 bg-white rounded-xl shadow-lg border border-stone-200
           p-2 z-50 min-w-[180px]"
    role="menu"
    aria-label={$t('nav.more')}
    onkeydown={handleMenuKeydown}
  >
    {#each moreItems as item, i}
      <button
        class="w-full text-left px-4 py-3 rounded-lg hover:bg-stone-50
               text-stone-700 text-sm min-h-[44px]
               {focusedIndex === i ? 'bg-stone-50' : ''}"
        role="menuitem"
        tabindex={focusedIndex === i ? 0 : -1}
        onclick={() => {
          showMore = false;
          navigation.navigate(item.id);
        }}
      >
        {$t(item.key)}
      </button>
    {/each}
  </div>
{/if}
