<!-- D6: Breadcrumb trail for nested screens using Flowbite Breadcrumb. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { Breadcrumb, BreadcrumbItem } from 'flowbite-svelte';
  import { navigation } from '$lib/stores/navigation.svelte';

  /** Map nested screens to their parent chains. */
  const chains: Record<string, string[]> = {
    'document-detail': ['documents'],
    'review': ['documents'],
    'ai-settings': ['settings'],
    'ai-setup': ['settings', 'ai-settings'],
    'privacy': ['settings'],
    'pairing': ['settings'],
    'import': ['documents'],
    'transfer': ['settings'],
  };

  let crumbs = $derived.by(() => {
    const chain = chains[navigation.activeScreen];
    if (!chain) return [];
    return [
      ...chain.map(s => ({ label: $t(`nav.${s}`), screen: s })),
      { label: $t(`nav.${navigation.activeScreen}`) ?? navigation.activeScreen, screen: undefined as string | undefined },
    ];
  });
</script>

{#if crumbs.length > 0}
  <Breadcrumb class="text-sm" ariaLabel={$t('nav.breadcrumb')}>
    {#each crumbs as crumb, i}
      <BreadcrumbItem
        home={i === 0}
        href={crumb.screen ? undefined : undefined}
        class="dark:text-gray-400"
      >
        {#if crumb.screen}
          <button
            class="hover:text-[var(--color-primary)] hover:underline"
            onclick={() => navigation.navigate(crumb.screen!)}
          >
            {crumb.label}
          </button>
        {:else}
          <span class="text-stone-800 dark:text-gray-200 font-medium">{crumb.label}</span>
        {/if}
      </BreadcrumbItem>
    {/each}
  </Breadcrumb>
{/if}
