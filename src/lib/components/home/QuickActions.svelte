<script lang="ts">
  interface Props {
    hasDocuments: boolean;
    onNavigate: (screen: string) => void;
  }
  let { hasDocuments, onNavigate }: Props = $props();

  const actions = [
    { id: 'import', label: '+ Document', sublabel: 'Load a file', primary: true },
    { id: 'chat', label: 'Ask', sublabel: 'Ask a question', primary: false },
    { id: 'journal', label: 'Journal', sublabel: 'How are you feeling?', primary: false },
  ];
</script>

<div class="px-6 py-3">
  <div class="grid grid-cols-3 gap-3">
    {#each actions as action}
      <button
        class="flex flex-col items-center justify-center gap-1 p-4 rounded-xl
               min-h-[80px] transition-colors
               {!hasDocuments && action.primary
                 ? 'bg-teal-600 text-white shadow-md'
                 : 'bg-white text-stone-700 border border-stone-200 hover:bg-stone-50'}"
        onclick={() => onNavigate(action.id)}
      >
        <span class="font-medium text-sm">{action.label}</span>
        <span class="text-xs opacity-70">{action.sublabel}</span>
      </button>
    {/each}
  </div>
</div>
