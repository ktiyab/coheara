<script lang="ts">
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

  const moreItems = [
    { id: 'documents', key: 'nav.documents' },
    { id: 'timeline', key: 'nav.timeline' },
    { id: 'appointments', key: 'nav.appointments' },
    { id: 'settings', key: 'nav.settings' },
  ];
</script>

<nav
  class="fixed bottom-0 left-0 right-0 bg-white border-t border-stone-200
         flex items-center justify-around h-16 z-50"
>
  {#each tabs as tab}
    <button
      class="flex flex-col items-center justify-center gap-1 flex-1 h-full
             min-h-[44px] min-w-[44px]
             {navigation.activeTab === tab.id ? 'text-teal-600' : 'text-stone-400'}"
      onclick={() => {
        if (tab.id === 'more') {
          showMore = !showMore;
        } else {
          showMore = false;
          navigation.navigate(tab.id);
        }
      }}
      aria-current={navigation.activeTab === tab.id ? 'page' : undefined}
      aria-label={$t(tab.key)}
    >
      <span class="text-lg">{tab.icon}</span>
      <span class="text-xs">{$t(tab.key)}</span>
    </button>
  {/each}
</nav>

{#if showMore}
  <div
    class="fixed bottom-16 right-2 bg-white rounded-xl shadow-lg border border-stone-200
           p-2 z-50 min-w-[180px]"
    role="menu"
  >
    {#each moreItems as item}
      <button
        class="w-full text-left px-4 py-3 rounded-lg hover:bg-stone-50
               text-stone-700 text-sm min-h-[44px]"
        role="menuitem"
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
