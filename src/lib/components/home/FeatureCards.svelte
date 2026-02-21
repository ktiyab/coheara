<!-- V8-B6: Feature teaching cards — replace QuickActions with substantive guidance -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { FileSearchOutline, MessagesOutline, HeartOutline } from 'flowbite-svelte-icons';
  import Button from '$lib/components/ui/Button.svelte';

  interface Props {
    hasDocuments: boolean;
  }
  let { hasDocuments }: Props = $props();

  const cards = [
    {
      icon: FileSearchOutline,
      titleKey: 'home.feature_documents_title',
      bodyKey: 'home.feature_documents_body',
      ctaKey: 'home.feature_documents_cta',
      action: 'import',
      primary: true,
    },
    {
      icon: MessagesOutline,
      titleKey: 'home.feature_chat_title',
      bodyKey: 'home.feature_chat_body',
      ctaKey: 'home.feature_chat_cta',
      action: 'chat',
      primary: false,
    },
    {
      icon: HeartOutline,
      titleKey: 'home.feature_journal_title',
      bodyKey: 'home.feature_journal_body',
      ctaKey: 'home.feature_journal_cta',
      action: 'journal',
      primary: false,
    },
  ];
</script>

<div class="px-6 mt-4">
  <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
    {#each cards as card}
      <div class="bg-white dark:bg-gray-900 border border-stone-200 dark:border-gray-700 rounded-xl p-6 flex flex-col">
        <!-- Icon -->
        <div class="w-10 h-10 rounded-lg flex items-center justify-center mb-3
                    {!hasDocuments && card.primary
                      ? 'bg-[var(--color-interactive-50)] text-[var(--color-interactive)]'
                      : 'bg-stone-100 dark:bg-gray-800 text-stone-500 dark:text-gray-400'}">
          <card.icon class="w-5 h-5" />
        </div>

        <!-- Title -->
        <h3 class="text-sm font-semibold text-stone-800 dark:text-gray-100 mb-1">
          {$t(card.titleKey)}
        </h3>

        <!-- Body — hidden in compact mode (when user has documents) -->
        {#if !hasDocuments}
          <p class="text-xs text-stone-500 dark:text-gray-400 leading-relaxed mb-4 flex-1">
            {$t(card.bodyKey)}
          </p>
        {:else}
          <div class="flex-1"></div>
        {/if}

        <!-- CTA -->
        <div class="mt-auto pt-2">
          <Button
            variant={!hasDocuments && card.primary ? 'primary' : 'ghost'}
            size="sm"
            fullWidth
            onclick={() => navigation.navigate(card.action)}
          >
            {$t(card.ctaKey)}
          </Button>
        </div>
      </div>
    {/each}
  </div>
</div>
