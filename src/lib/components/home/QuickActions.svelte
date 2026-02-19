<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';

  interface Props {
    hasDocuments: boolean;
  }
  let { hasDocuments }: Props = $props();

  const actionDefs = [
    { id: 'import', labelKey: 'home.action_import_label', sublabelKey: 'home.action_import_sublabel', primary: true },
    { id: 'chat', labelKey: 'home.action_chat_label', sublabelKey: 'home.action_chat_sublabel', primary: false },
    { id: 'journal', labelKey: 'home.action_journal_label', sublabelKey: 'home.action_journal_sublabel', primary: false },
  ];
</script>

<div class="px-6 py-3">
  <div class="grid grid-cols-3 gap-3">
    {#each actionDefs as action}
      <button
        class="flex flex-col items-center justify-center gap-1 p-4 rounded-xl
               min-h-[80px] transition-colors
               {!hasDocuments && action.primary
                 ? 'bg-[var(--color-interactive)] text-white shadow-md'
                 : 'bg-white text-stone-700 border border-stone-200 hover:bg-stone-50'}"
        onclick={() => navigation.navigate(action.id)}
      >
        <span class="font-medium text-sm">{$t(action.labelKey)}</span>
        <span class="text-xs opacity-70">{$t(action.sublabelKey)}</span>
      </button>
    {/each}
  </div>
</div>
