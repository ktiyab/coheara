<!-- UX-03: Privacy & Data Screen — redesigned to match SettingsScreen hub pattern. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import { getPrivacyInfo, openDataFolder } from '$lib/api/trust';
  import type { PrivacyInfo } from '$lib/types/trust';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { profile } from '$lib/stores/profile.svelte';
  import { theme } from '$lib/stores/theme.svelte';
  import { PRIVACY_HUES, colorfulStyle } from '$lib/theme/colorful-mappings';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import BackButton from '$lib/components/ui/BackButton.svelte';
  import BackupRestoreSection from './BackupRestoreSection.svelte';
  import DataSharingSection from './DataSharingSection.svelte';
  import DeleteProfileSection from './DeleteProfileSection.svelte';
  import {
    DocsIcon, LockIcon, ChevronRightIcon,
  } from '$lib/components/icons/md';

  let privacyInfo: PrivacyInfo | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);

  onMount(async () => {
    try {
      privacyInfo = await getPrivacyInfo();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  });

  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  }
</script>

<div class="flex flex-col bg-stone-50 dark:bg-gray-950 min-h-full">
  <!-- Header with back button -->
  <header class="px-[var(--spacing-page-x)] pt-6 pb-4 flex items-center gap-3">
    <BackButton />
    <h1 class="text-2xl font-bold text-stone-800 dark:text-gray-100">
      {$t('settings.hub_privacy_title')}
    </h1>
  </header>

  {#if loading}
    <LoadingState message={$t('common.loading')} />
  {:else if error}
    <div class="px-[var(--spacing-page-x)]">
      <div class="bg-[var(--color-danger-50)] rounded-[var(--radius-card)] p-5 border border-[var(--color-danger-200)]">
        <p class="text-sm text-[var(--color-danger-800)]">{error}</p>
        <button
          class="mt-3 px-4 py-2 bg-white dark:bg-gray-900 border border-[var(--color-danger-200)] rounded-lg text-sm
                 text-[var(--color-danger)] min-h-[44px]"
          onclick={async () => {
            loading = true;
            error = null;
            try {
              privacyInfo = await getPrivacyInfo();
            } catch (e) {
              error = e instanceof Error ? e.message : String(e);
            } finally {
              loading = false;
            }
          }}
        >
          {$t('common.retry')}
        </button>
      </div>
    </div>
  {:else if privacyInfo}
    <div class="px-[var(--spacing-page-x)] pb-6 space-y-4">

      <!-- ═══ Section 1: Your Data ═══ -->
      <section>
        <div style={theme.isColorful ? colorfulStyle(PRIVACY_HUES[0]) : undefined}
             class="bg-white dark:bg-gray-900 rounded-[var(--radius-card)] border border-stone-100 dark:border-gray-800 shadow-sm divide-y divide-stone-100 dark:divide-gray-800">
          <!-- Header row -->
          <div class="flex items-center gap-4 px-4 py-3 min-h-[52px]">
            <DocsIcon class="w-9 h-9 text-[var(--color-success)] flex-shrink-0" />
            <span class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.your_data')}</span>
          </div>
          <!-- Location -->
          <div class="px-4 py-3 min-h-[52px] pl-[68px] flex items-center">
            <span class="text-sm text-stone-500 dark:text-gray-400">{$t('settings.location')}:</span>
            <span class="text-sm text-stone-800 dark:text-gray-100 font-mono text-xs ml-2 truncate"
                  title={privacyInfo.data_location}>
              {privacyInfo.data_location}
            </span>
          </div>
          <!-- Total Size -->
          <div class="px-4 py-3 min-h-[52px] pl-[68px] flex items-center">
            <span class="text-sm text-stone-500 dark:text-gray-400">{$t('settings.total_size')}:</span>
            <span class="text-sm text-stone-800 dark:text-gray-100 font-medium ml-2">{formatBytes(privacyInfo.total_data_size_bytes)}</span>
          </div>
          <!-- Documents -->
          <div class="px-4 py-3 min-h-[52px] pl-[68px] flex items-center">
            <span class="text-sm text-stone-500 dark:text-gray-400">{$t('settings.documents')}:</span>
            <span class="text-sm text-stone-800 dark:text-gray-100 font-medium ml-2">{privacyInfo.document_count}</span>
          </div>
          <!-- Last Backup -->
          <div class="px-4 py-3 min-h-[52px] pl-[68px] flex items-center">
            <span class="text-sm text-stone-500 dark:text-gray-400">{$t('settings.last_backup')}:</span>
            <span class="text-sm text-stone-800 dark:text-gray-100 font-medium ml-2">
              {privacyInfo.last_backup_date
                ? new Date(privacyInfo.last_backup_date).toLocaleDateString()
                : $t('common.never')}
            </span>
          </div>
          <!-- Open Data Folder — action row -->
          <button
            class="w-full flex items-center gap-4 px-4 py-3 min-h-[52px] pl-[68px] text-left
                   hover:bg-stone-50 dark:hover:bg-gray-800 transition-colors last:rounded-b-xl"
            onclick={async () => {
              try { await openDataFolder(); } catch (e) { console.error('Failed to open data folder:', e); }
            }}
          >
            <span class="text-sm text-[var(--color-success)] font-medium flex-1">{$t('settings.open_data_folder')}</span>
            <ChevronRightIcon class="w-5 h-5 text-[var(--color-success)] flex-shrink-0" />
          </button>
        </div>
      </section>

      <!-- ═══ Section 2: Security ═══ -->
      <section>
        <div style={theme.isColorful ? colorfulStyle(PRIVACY_HUES[1]) : undefined}
             class="bg-white dark:bg-gray-900 rounded-[var(--radius-card)] border border-stone-100 dark:border-gray-800 shadow-sm divide-y divide-stone-100 dark:divide-gray-800">
          <!-- Header row -->
          <div class="flex items-center gap-4 px-4 py-3 min-h-[52px]">
            <LockIcon class="w-9 h-9 text-[var(--color-success)] flex-shrink-0" />
            <span class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.security')}</span>
          </div>
          <!-- Encryption -->
          <div class="px-4 py-3 min-h-[52px] pl-[68px] flex items-center">
            <span class="text-sm text-stone-500 dark:text-gray-400">{$t('settings.encryption')}:</span>
            <span class="text-sm text-stone-800 dark:text-gray-100 font-medium ml-2">{privacyInfo.encryption_algorithm}</span>
          </div>
          <!-- Key Derivation -->
          <div class="px-4 py-3 min-h-[52px] pl-[68px] flex items-center">
            <span class="text-sm text-stone-500 dark:text-gray-400">{$t('settings.key_derivation')}:</span>
            <span class="text-sm text-stone-800 dark:text-gray-100 font-medium ml-2">{privacyInfo.key_derivation}</span>
          </div>
        </div>
      </section>

      <!-- ═══ Section 3: Data Sharing ═══ -->
      <section>
        <div style={theme.isColorful ? colorfulStyle(PRIVACY_HUES[2]) : undefined}>
          <DataSharingSection />
        </div>
      </section>

      <!-- ═══ Section 4: Backup & Restore ═══ -->
      <section>
        <div style={theme.isColorful ? colorfulStyle(PRIVACY_HUES[3]) : undefined}>
          <BackupRestoreSection />
        </div>
      </section>

      <!-- ═══ Section 5: Delete Profile (danger zone) ═══ -->
      <DeleteProfileSection onDeleted={() => navigation.navigate('picker')} />

    </div>
  {/if}
</div>
