<!-- D6: Uniform app header — profile identity (left), AI status (right).
     Sidebar screens: no title (sidebar shows active page, each screen has its own h1).
     Nested screens: back button + breadcrumbs + title (navigation context). -->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { profile } from '$lib/stores/profile.svelte';
  import { PROFILE_COLORS } from '$lib/types/profile';
  import Breadcrumbs from './Breadcrumbs.svelte';
  import BackButton from '$lib/components/ui/BackButton.svelte';
  import Avatar from '$lib/components/ui/Avatar.svelte';
  import AiStatusIndicator from './AiStatusIndicator.svelte';

  interface Props {
    title?: string;
    actions?: Snippet;
    /** MP-02: When true, suppress the small managed-by subtitle (banner shows it instead). */
    hideManagedLabel?: boolean;
  }
  let { title, actions, hideManagedLabel = false }: Props = $props();

  let screenTitle = $derived(title ?? $t(`nav.${navigation.activeScreen}`) ?? navigation.activeScreen);

  /** Show back button on nested screens that aren't in the sidebar. */
  let showBack = $derived(!navigation.showSidebar);

  /** Profile color from 8-color palette. */
  let profileColor = $derived(
    profile.colorIndex != null
      ? PROFILE_COLORS[profile.colorIndex % PROFILE_COLORS.length]
      : null
  );
</script>

<header
  class="h-[var(--header-height)]
         bg-stone-50 dark:bg-gray-950 px-6 flex items-center justify-between flex-shrink-0"
>
  <!-- Left side -->
  <div class="flex items-center gap-3 min-w-0">
    {#if showBack}
      <!-- Nested screens: back + breadcrumbs + title -->
      <BackButton onclick={() => navigation.goBack()} />
      <Breadcrumbs />
      <h1 class="text-lg font-semibold text-stone-800 dark:text-gray-100 truncate">{screenTitle}</h1>
    {:else}
      <!-- Sidebar screens: profile indicator (data owner) + F6 managed-by badge -->
      <div class="flex items-center gap-2.5">
        <Avatar name={profile.name || 'P'} size="sm" color={profileColor} />
        <div class="flex flex-col min-w-0">
          <span class="text-sm font-medium text-stone-700 dark:text-gray-200 truncate max-w-[160px]">
            {profile.name || $t('common.patient')}
          </span>
          {#if profile.managedBy && !hideManagedLabel}
            <span class="text-xs text-amber-600 dark:text-amber-400 truncate max-w-[160px]">
              {$t('profile.viewing_managed', { values: { caregiver: profile.managedBy } })}
            </span>
          {/if}
        </div>
      </div>
    {/if}
  </div>

  <!-- Right side -->
  <div class="flex items-center gap-2 flex-shrink-0">
    {#if actions}
      {@render actions()}
    {/if}
    <AiStatusIndicator />
  </div>
</header>
