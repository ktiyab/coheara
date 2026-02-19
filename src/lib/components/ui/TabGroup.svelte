<!--
  C10: TabGroup — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C10
  Replaces: 2+ inline tab navigation implementations

  Pill-style tab bar with optional count badges.
  Full ARIA tablist pattern with keyboard navigation.
-->
<script lang="ts">
  interface Tab {
    value: string;
    label: string;
    count?: number;
  }

  interface Props {
    tabs: Tab[];
    selected: string;
    onselect: (value: string) => void;
  }

  let {
    tabs,
    selected,
    onselect,
  }: Props = $props();

  function handleKeydown(e: KeyboardEvent, index: number) {
    let nextIndex = index;
    if (e.key === 'ArrowRight') {
      nextIndex = (index + 1) % tabs.length;
    } else if (e.key === 'ArrowLeft') {
      nextIndex = (index - 1 + tabs.length) % tabs.length;
    } else if (e.key === 'Home') {
      nextIndex = 0;
    } else if (e.key === 'End') {
      nextIndex = tabs.length - 1;
    } else {
      return;
    }
    e.preventDefault();
    onselect(tabs[nextIndex].value);
    // Focus the activated tab button
    const tablist = (e.target as HTMLElement).closest('[role="tablist"]');
    const buttons = tablist?.querySelectorAll<HTMLButtonElement>('[role="tab"]');
    buttons?.[nextIndex]?.focus();
  }
</script>

<div class="flex gap-2 overflow-x-auto px-1 py-1" role="tablist">
  {#each tabs as tab, i}
    <button
      class="px-4 py-2 rounded-full text-sm font-medium whitespace-nowrap min-h-[44px]
             transition-colors
             {selected === tab.value
               ? 'bg-[var(--color-primary)] text-white'
               : 'bg-white text-stone-600 border border-stone-200 hover:bg-stone-50'}"
      role="tab"
      aria-selected={selected === tab.value}
      tabindex={selected === tab.value ? 0 : -1}
      onclick={() => onselect(tab.value)}
      onkeydown={(e) => handleKeydown(e, i)}
    >
      {tab.label}
      {#if tab.count !== undefined}
        <span class="ml-1.5 text-xs opacity-70">{tab.count}</span>
      {/if}
    </button>
  {/each}
</div>
