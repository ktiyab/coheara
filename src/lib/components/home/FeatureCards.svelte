<!-- V8-B6 + AUDIT_01 §3: Feature teaching cards — 2-col grid, left-aligned, status line. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { DocsIcon, SearchIcon, TimelineIcon } from '$lib/components/icons/md';
  import Button from '$lib/components/ui/Button.svelte';
  import StatusDot from '$lib/components/ui/StatusDot.svelte';
  import type { Component } from 'svelte';

  interface Props {
    hasDocuments: boolean;
  }
  let { hasDocuments }: Props = $props();

  type FeatureCard = {
    icon: Component<{ class?: string }>;
    titleKey: string;
    bodyKey: string;
    ctaKey: string;
    action: string;
    primary: boolean;
    statusKey: string;
  };

  const cards: FeatureCard[] = [
    {
      icon: DocsIcon,
      titleKey: 'home.feature_documents_title',
      bodyKey: 'home.feature_documents_body',
      ctaKey: 'home.feature_documents_cta',
      action: 'import',
      primary: true,
      statusKey: 'home.feature_documents_status',
    },
    {
      icon: SearchIcon,
      titleKey: 'home.feature_chat_title',
      bodyKey: 'home.feature_chat_body',
      ctaKey: 'home.feature_chat_cta',
      action: 'chat',
      primary: false,
      statusKey: 'home.feature_chat_status',
    },
    {
      icon: TimelineIcon,
      titleKey: 'home.feature_timeline_title',
      bodyKey: 'home.feature_timeline_body',
      ctaKey: 'home.feature_timeline_cta',
      action: 'timeline',
      primary: false,
      statusKey: 'home.feature_timeline_status',
    },
  ];
</script>

<div class="px-[var(--spacing-page-x)] mt-4">
  <div class="grid grid-cols-1 md:grid-cols-2 gap-[var(--spacing-grid)]">
    {#each cards as card, i}
      <div
        class="bg-white dark:bg-gray-900 border border-stone-200 dark:border-gray-700
               rounded-[var(--radius-card)] p-[var(--spacing-card)] flex flex-col
               {i === cards.length - 1 ? 'md:col-span-2' : ''}"
      >
        <!-- Icon — top-left (AUDIT_01 §3B) -->
        <div class="w-10 h-10 rounded-lg flex items-center justify-center mb-4
                    {!hasDocuments && card.primary
                      ? 'bg-[var(--color-interactive-50)] text-[var(--color-interactive)]'
                      : 'bg-stone-100 dark:bg-gray-800 text-stone-500 dark:text-gray-400'}">
          <card.icon class="w-5 h-5" />
        </div>

        <!-- Title — left-aligned -->
        <h3 class="text-[var(--text-card-title)] font-semibold text-stone-800 dark:text-gray-100 mb-1">
          {$t(card.titleKey)}
        </h3>

        <!-- Body — always visible (AUDIT_01 §3B: description always shown) -->
        <p class="text-[var(--text-body)] text-stone-500 dark:text-gray-400 leading-relaxed mb-4 flex-1">
          {$t(card.bodyKey)}
        </p>

        <!-- CTA — filled for primary (AUDIT_01 §4) -->
        <div>
          <Button
            variant={card.primary ? 'primary' : 'ghost'}
            size="sm"
            onclick={() => navigation.navigate(card.action)}
          >
            {$t(card.ctaKey)}
          </Button>
        </div>

        <!-- Separator + status line (AUDIT_01 §3B) -->
        <div class="border-t border-stone-100 dark:border-gray-800 mt-4 pt-3">
          <StatusDot
            label={$t(card.statusKey) ?? (hasDocuments ? 'Ready' : 'Get started')}
            variant={hasDocuments ? 'success' : 'neutral'}
          />
        </div>
      </div>
    {/each}
  </div>
</div>
