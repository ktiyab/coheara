<!-- L5-01: Privacy & Data Screen — main settings/trust screen -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { getPrivacyInfo, openDataFolder } from '$lib/api/trust';
  import type { PrivacyInfo } from '$lib/types/trust';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { profile } from '$lib/stores/profile.svelte';
  import BackupRestoreSection from './BackupRestoreSection.svelte';
  import DeleteProfileSection from './DeleteProfileSection.svelte';

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

<div class="flex flex-col min-h-screen pb-20 bg-stone-50">
  <header class="px-6 pt-6 pb-4">
    <h1 class="text-2xl font-bold text-stone-800">Privacy & Data</h1>
  </header>

  {#if loading}
    <div class="flex-1 flex items-center justify-center">
      <div class="animate-pulse text-stone-400">Loading...</div>
    </div>
  {:else if error}
    <div class="px-6">
      <div class="bg-red-50 rounded-xl p-5 border border-red-200">
        <p class="text-sm text-red-700">{error}</p>
        <button
          class="mt-3 px-4 py-2 bg-white border border-red-200 rounded-lg text-sm
                 text-red-600 min-h-[44px]"
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
          Retry
        </button>
      </div>
    </div>
  {:else if privacyInfo}
    <div class="px-6 space-y-4">
      <!-- Your Data -->
      <section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
        <h2 class="text-sm font-medium text-stone-500 mb-3">YOUR DATA</h2>
        <div class="space-y-2 text-sm">
          <div class="flex justify-between">
            <span class="text-stone-600">Location</span>
            <span
              class="text-stone-800 font-mono text-xs max-w-[200px] truncate"
              title={privacyInfo.data_location}
            >
              {privacyInfo.data_location}
            </span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600">Total size</span>
            <span class="text-stone-800">{formatBytes(privacyInfo.total_data_size_bytes)}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600">Documents</span>
            <span class="text-stone-800">{privacyInfo.document_count}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600">Last backup</span>
            <span class="text-stone-800">
              {privacyInfo.last_backup_date
                ? new Date(privacyInfo.last_backup_date).toLocaleDateString()
                : 'Never'}
            </span>
          </div>
        </div>
      </section>

      <!-- Security -->
      <section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
        <h2 class="text-sm font-medium text-stone-500 mb-3">SECURITY</h2>
        <div class="space-y-2 text-sm">
          <div class="flex justify-between">
            <span class="text-stone-600">Encryption</span>
            <span class="text-stone-800">{privacyInfo.encryption_algorithm}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600">Key derivation</span>
            <span class="text-stone-800">{privacyInfo.key_derivation}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600">Network access</span>
            <span class="text-green-700 font-medium">{privacyInfo.network_permissions}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-stone-600">Tracking</span>
            <span class="text-green-700 font-medium">{privacyInfo.telemetry}</span>
          </div>
        </div>
      </section>

      <!-- AI Engine -->
      <section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
        <h2 class="text-sm font-medium text-stone-500 mb-3">AI ENGINE</h2>
        <div class="space-y-2 text-sm">
          <div class="flex justify-between">
            <span class="text-stone-600">Status</span>
            <span class={profile.isAiAvailable ? 'text-green-700 font-medium' : 'text-amber-700'}>
              {profile.isAiAvailable ? 'Ready' : 'Not configured'}
            </span>
          </div>
          {#if profile.aiStatus?.ollama_model}
            <div class="flex justify-between">
              <span class="text-stone-600">Model</span>
              <span class="text-stone-800">{profile.aiStatus.ollama_model}</span>
            </div>
          {/if}
        </div>
        <div class="flex gap-3 mt-3">
          {#if !profile.isAiAvailable}
            <button
              class="flex-1 px-4 py-2 bg-teal-600 text-white rounded-lg text-sm font-medium hover:bg-teal-700 min-h-[44px]"
              onclick={() => navigation.navigate('ai-setup')}
            >
              Set up AI
            </button>
          {/if}
          <button
            class="flex-1 px-4 py-2 border border-stone-200 rounded-lg text-sm text-stone-600 hover:bg-stone-50 min-h-[44px]"
            onclick={() => navigation.navigate('ai-settings')}
          >
            AI settings
          </button>
        </div>
      </section>

      <!-- Verify It Yourself -->
      <section class="bg-blue-50 rounded-xl p-5 border border-blue-100">
        <h2 class="text-sm font-medium text-blue-700 mb-3">VERIFY IT YOURSELF</h2>
        <ol class="text-sm text-blue-800 space-y-1 list-decimal list-inside">
          <li>Turn on airplane mode</li>
          <li>Open Coheara</li>
          <li>Everything works exactly the same</li>
        </ol>
        <p class="text-xs text-blue-600 mt-2">This proves no internet connection is needed.</p>
      </section>

      <!-- Quick Actions -->
      <div class="flex gap-3">
        <button
          class="flex-1 px-4 py-3 bg-white border border-stone-200 rounded-xl
                 text-sm font-medium text-stone-700 min-h-[44px]"
          onclick={async () => {
            try {
              await openDataFolder();
            } catch (e) {
              console.error('Failed to open data folder:', e);
            }
          }}
        >
          Open data folder
        </button>
      </div>

      <!-- Backup & Restore -->
      <BackupRestoreSection />

      <!-- Delete Profile (danger zone) -->
      <DeleteProfileSection onDeleted={() => navigation.navigate('picker')} />
    </div>
  {/if}
</div>
