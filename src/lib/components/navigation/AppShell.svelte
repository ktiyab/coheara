<!-- D6: App shell — grid layout with sidebar + content area. Replaces TabBar layout. -->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { profile } from '$lib/stores/profile.svelte';
  import { profiles } from '$lib/stores/profiles.svelte';
  import { lockProfile } from '$lib/api/profile';
  import { dispatchProfileSwitch } from '$lib/utils/session-events';
  import AppSidebar from './AppSidebar.svelte';
  import AppHeader from './AppHeader.svelte';
  import ViewingAsBanner from './ViewingAsBanner.svelte';

  interface Props {
    children: Snippet;
  }
  let { children }: Props = $props();

  /** MP-02: Show banner when viewing a managed profile's data. */
  let isManaged = $derived(profile.managedBy !== null);

  /** MP-02: Return to caregiver's profile from managed profile view. */
  async function handleReturnToSelf() {
    const caregiverName = profile.managedBy;
    if (!caregiverName) return;
    const caregiver = profiles.all.find((p) => p.name === caregiverName);
    await lockProfile();
    dispatchProfileSwitch(caregiver?.id);
  }

  /** Focus main content when screen changes. */
  $effect(() => {
    navigation.activeScreen;
    const main = document.getElementById('main-content');
    main?.focus();
  });

  /** Ctrl+B toggles sidebar. */
  function handleKeydown(e: KeyboardEvent) {
    if (e.ctrlKey && e.key === 'b') {
      e.preventDefault();
      navigation.toggleSidebar();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Skip-to-content link (ACC-00-12) -->
<a
  href="#main-content"
  class="sr-only focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-50
         focus:bg-white dark:focus:bg-gray-800 focus:px-4 focus:py-2 focus:rounded-lg focus:shadow-lg
         focus:text-[var(--color-primary)] focus:font-medium"
  onclick={(e) => { e.preventDefault(); document.getElementById('main-content')?.focus(); }}
>
  {$t('nav.skip_to_content') ?? 'Skip to main content'}
</a>

{#if navigation.showSidebar}
  <div class="grid grid-cols-[auto_1fr] h-screen bg-stone-50 dark:bg-gray-950">
    <AppSidebar />

    <div class="flex flex-col min-h-0 overflow-hidden">
      {#if isManaged}
        <ViewingAsBanner managedName={profile.name} onReturnToSelf={handleReturnToSelf} />
      {/if}
      <AppHeader hideManagedLabel={isManaged} />
      <main
        id="main-content"
        tabindex="-1"
        class="flex-1 overflow-y-auto outline-none"
        aria-label={$t(`nav.${navigation.activeScreen}`) ?? navigation.activeScreen}
      >
        {@render children()}
      </main>
    </div>
  </div>
{:else}
  <!-- Nested/sub-screens: no sidebar, full width with header -->
  <div class="flex flex-col h-screen bg-stone-50 dark:bg-gray-950">
    {#if isManaged}
      <ViewingAsBanner managedName={profile.name} onReturnToSelf={handleReturnToSelf} />
    {/if}
    <AppHeader hideManagedLabel={isManaged} />
    <main
      id="main-content"
      tabindex="-1"
      class="flex-1 overflow-y-auto outline-none"
      aria-label={$t(`nav.${navigation.activeScreen}`) ?? navigation.activeScreen}
    >
      {@render children()}
    </main>
  </div>
{/if}
