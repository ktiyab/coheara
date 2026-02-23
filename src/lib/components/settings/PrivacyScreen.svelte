<!-- L5-01: Privacy & Data Screen — main settings/trust screen -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import { getPrivacyInfo, openDataFolder } from '$lib/api/trust';
  import type { PrivacyInfo } from '$lib/types/trust';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { profile } from '$lib/stores/profile.svelte';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import BackupRestoreSection from './BackupRestoreSection.svelte';
  import DataSharingSection from './DataSharingSection.svelte';
  import DeleteProfileSection from './DeleteProfileSection.svelte';
  import LanguageSelector from './LanguageSelector.svelte';

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

<div class="flex flex-col bg-stone-50 dark:bg-gray-950">
  {#if loading}
    <LoadingState message={$t('common.loading')} />
  {:else if error}
    <div class="px-6">
      <div class="bg-[var(--color-danger-50)] rounded-xl p-5 border border-[var(--color-danger-200)]">
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
    <div class="px-6 space-y-4">
      <!-- Language Selector (I18N-38) -->
      <section class="bg-white dark:bg-gray-900 rounded-xl p-5 border border-stone-100 dark:border-gray-800 shadow-sm">
        <LanguageSelector />
      </section>

      <!-- Your Data -->
      <section class="bg-white dark:bg-gray-900 rounded-xl p-5 border border-stone-100 dark:border-gray-800 shadow-sm">
        <h2 class="text-sm font-medium text-stone-500 dark:text-gray-400 mb-3">{$t('settings.your_data')}</h2>
        <div class="space-y-2 text-sm">
          <div class="flex justify-between">
            <span class="text-stone-600 dark:text-gray-300">{$t('settings.location')}</span>
            <span
              class="text-stone-800 dark:text-gray-100 font-mono text-xs max-w-[200px] truncate"
              title={privacyInfo.data_location}
            >
              {privacyInfo.data_location}
            </span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600 dark:text-gray-300">{$t('settings.total_size')}</span>
            <span class="text-stone-800 dark:text-gray-100">{formatBytes(privacyInfo.total_data_size_bytes)}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600 dark:text-gray-300">{$t('settings.documents')}</span>
            <span class="text-stone-800 dark:text-gray-100">{privacyInfo.document_count}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600 dark:text-gray-300">{$t('settings.last_backup')}</span>
            <span class="text-stone-800 dark:text-gray-100">
              {privacyInfo.last_backup_date
                ? new Date(privacyInfo.last_backup_date).toLocaleDateString()
                : $t('common.never')}
            </span>
          </div>
        </div>
      </section>

      <!-- Security -->
      <section class="bg-white dark:bg-gray-900 rounded-xl p-5 border border-stone-100 dark:border-gray-800 shadow-sm">
        <h2 class="text-sm font-medium text-stone-500 dark:text-gray-400 mb-3">{$t('settings.security')}</h2>
        <div class="space-y-2 text-sm">
          <div class="flex justify-between">
            <span class="text-stone-600 dark:text-gray-300">{$t('settings.encryption')}</span>
            <span class="text-stone-800 dark:text-gray-100">{privacyInfo.encryption_algorithm}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600 dark:text-gray-300">{$t('settings.key_derivation')}</span>
            <span class="text-stone-800 dark:text-gray-100">{privacyInfo.key_derivation}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600 dark:text-gray-300">{$t('settings.network_access')}</span>
            <span class="text-[var(--color-success)] font-medium">{privacyInfo.network_permissions}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600 dark:text-gray-300">{$t('settings.tracking')}</span>
            <span class="text-[var(--color-success)] font-medium">{privacyInfo.telemetry}</span>
          </div>
        </div>
      </section>

      <!-- MP-02: Data Sharing -->
      <DataSharingSection />

      <!-- AI Engine -->
      <section class="bg-white dark:bg-gray-900 rounded-xl p-5 border border-stone-100 dark:border-gray-800 shadow-sm">
        <h2 class="text-sm font-medium text-stone-500 dark:text-gray-400 mb-3">{$t('settings.ai_engine')}</h2>
        <div class="space-y-2 text-sm">
          <div class="flex justify-between">
            <span class="text-stone-600 dark:text-gray-300">{$t('settings.ai_status')}</span>
            <span class={profile.isAiAvailable ? 'text-[var(--color-success)] font-medium' : 'text-[var(--color-warning-800)]'}>
              {profile.isAiAvailable ? $t('settings.ai_ready') : $t('settings.ai_not_configured')}
            </span>
          </div>
          {#if profile.aiStatus?.active_model}
            <div class="flex justify-between">
              <span class="text-stone-600 dark:text-gray-300">{$t('settings.ai_model')}</span>
              <span class="text-stone-800 dark:text-gray-100">
                {profile.aiStatus.active_model.name}
                {#if profile.aiStatus.active_model.quality === 'Medical'}
                  <span class="text-xs text-[var(--color-interactive)] ml-1">{$t('settings.ai_medical')}</span>
                {:else}
                  <span class="text-xs text-stone-500 dark:text-gray-400 ml-1">{$t('settings.ai_general')}</span>
                {/if}
              </span>
            </div>
          {/if}
        </div>
        <div class="flex gap-3 mt-3">
          {#if !profile.isAiAvailable}
            <Button variant="primary" size="sm" onclick={() => navigation.navigate('ai-setup')}>
              {$t('settings.ai_setup')}
            </Button>
          {/if}
          <Button variant="secondary" size="sm" onclick={() => navigation.navigate('ai-settings')}>
            {$t('settings.ai_settings')}
          </Button>
        </div>
      </section>

      <!-- Verify It Yourself -->
      <section class="bg-[var(--color-info-50)] rounded-xl p-5 border border-[var(--color-info-200)]">
        <h2 class="text-sm font-medium text-[var(--color-info)] mb-3">{$t('settings.verify_yourself')}</h2>
        <ol class="text-sm text-[var(--color-info-800)] space-y-1 list-decimal list-inside">
          <li>{$t('settings.verify_step1')}</li>
          <li>{$t('settings.verify_step2')}</li>
          <li>{$t('settings.verify_step3')}</li>
        </ol>
        <p class="text-xs text-[var(--color-info)] mt-2">{$t('settings.verify_explanation')}</p>
      </section>

      <!-- Quick Actions -->
      <div class="flex gap-3">
        <Button variant="secondary" fullWidth onclick={async () => {
            try {
              await openDataFolder();
            } catch (e) {
              console.error('Failed to open data folder:', e);
            }
          }}>
          {$t('settings.open_data_folder')}
        </Button>
      </div>

      <!-- Backup & Restore -->
      <BackupRestoreSection />

      <!-- Delete Profile (danger zone) -->
      <DeleteProfileSection onDeleted={() => navigation.navigate('picker')} />
    </div>
  {/if}
</div>
