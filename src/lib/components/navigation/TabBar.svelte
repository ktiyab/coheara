<script lang="ts">
  interface Props {
    activeTab: string;
    onNavigate: (tab: string) => void;
  }
  let { activeTab, onNavigate }: Props = $props();

  const tabs = [
    { id: 'home', label: 'Home', icon: '⌂' },
    { id: 'chat', label: 'Chat', icon: '💬' },
    { id: 'journal', label: 'Journal', icon: '♡' },
    { id: 'medications', label: 'Meds', icon: '💊' },
    { id: 'more', label: 'More', icon: '⋯' },
  ];

  let showMore = $state(false);

  const moreItems = [
    { id: 'documents', label: 'Documents' },
    { id: 'timeline', label: 'Timeline' },
    { id: 'appointments', label: 'Appointments' },
    { id: 'settings', label: 'Settings' },
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
             {activeTab === tab.id ? 'text-teal-600' : 'text-stone-400'}"
      onclick={() => {
        if (tab.id === 'more') {
          showMore = !showMore;
        } else {
          showMore = false;
          onNavigate(tab.id);
        }
      }}
      aria-current={activeTab === tab.id ? 'page' : undefined}
      aria-label={tab.label}
    >
      <span class="text-lg">{tab.icon}</span>
      <span class="text-xs">{tab.label}</span>
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
          onNavigate(item.id);
        }}
      >
        {item.label}
      </button>
    {/each}
  </div>
{/if}
