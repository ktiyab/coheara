<!-- MP-02: Data Sharing Section — 1Password Families vault permissions pattern.
     Shows who can view your data + data you can view + grant/revoke controls. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import {
    listMyGrants,
    listGrantsToMe,
    grantProfileAccess,
    revokeProfileAccess,
    type EnrichedGrant
  } from '$lib/api/companion';
  import { profile } from '$lib/stores/profile.svelte';
  import { profiles } from '$lib/stores/profiles.svelte';
  import { PROFILE_COLORS } from '$lib/types/profile';

  let grantsOut = $state<EnrichedGrant[]>([]);
  let grantsIn = $state<EnrichedGrant[]>([]);
  let loading = $state(true);
  let error: string | null = $state(null);
  let successMsg: string | null = $state(null);

  // Grant form state (self-managed only)
  let selectedProfileId = $state('');
  let selectedAccessLevel = $state<'full' | 'read_only'>('read_only');
  let granting = $state(false);

  // Revoke confirmation
  let revokeTarget: EnrichedGrant | null = $state(null);
  let revoking = $state(false);

  /** Profiles eligible to receive a grant (exclude self + already granted). */
  let grantableProfiles = $derived(
    profiles.all.filter((p) => {
      if (!profile.activeInfo) return false;
      if (p.id === profile.activeInfo.id) return false;
      return !grantsOut.some((g) => g.grantee_profile_id === p.id);
    })
  );

  async function loadGrants() {
    loading = true;
    error = null;
    try {
      [grantsOut, grantsIn] = await Promise.all([listMyGrants(), listGrantsToMe()]);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function handleGrant() {
    if (!selectedProfileId || !profile.activeInfo) return;
    granting = true;
    error = null;
    try {
      await grantProfileAccess(profile.activeInfo.id, selectedProfileId, selectedAccessLevel);
      const grantedProfile = profiles.all.find((p) => p.id === selectedProfileId);
      successMsg = $t('settings.data_sharing_granted_msg', {
        values: { name: grantedProfile?.name ?? selectedProfileId }
      });
      selectedProfileId = '';
      await loadGrants();
      setTimeout(() => { successMsg = null; }, 3000);
    } catch (e) {
      error = $t('settings.data_sharing_error') ?? (e instanceof Error ? e.message : String(e));
    } finally {
      granting = false;
    }
  }

  async function handleRevoke(grant: EnrichedGrant) {
    revoking = true;
    error = null;
    try {
      await revokeProfileAccess(grant.granter_profile_id, grant.grantee_profile_id);
      successMsg = $t('settings.data_sharing_revoked_msg');
      revokeTarget = null;
      await loadGrants();
      setTimeout(() => { successMsg = null; }, 3000);
    } catch (e) {
      error = $t('settings.data_sharing_error') ?? (e instanceof Error ? e.message : String(e));
    } finally {
      revoking = false;
    }
  }

  function profileInitial(name: string): string {
    return (name || 'P').charAt(0).toUpperCase();
  }

  function profileColor(name: string): string {
    const idx = name.charCodeAt(0) % PROFILE_COLORS.length;
    return PROFILE_COLORS[idx];
  }

  function formatDate(dateStr: string): string {
    try {
      return new Date(dateStr).toLocaleDateString();
    } catch {
      return dateStr;
    }
  }

  onMount(loadGrants);
</script>

<section class="bg-white dark:bg-gray-900 rounded-xl p-5 border border-stone-100 dark:border-gray-800 shadow-sm">
  <h2 class="text-sm font-medium text-stone-500 dark:text-gray-400 mb-4">
    {$t('settings.data_sharing_heading')}
  </h2>

  {#if loading}
    <p class="text-sm text-stone-400 dark:text-gray-500">{$t('common.loading')}</p>
  {:else}
    <!-- Success message -->
    {#if successMsg}
      <div class="bg-[var(--color-success-50)] rounded-lg p-3 mb-3 border border-[var(--color-success-50)]">
        <p class="text-sm text-[var(--color-success)]">{successMsg}</p>
      </div>
    {/if}

    <!-- Error message -->
    {#if error}
      <div class="bg-[var(--color-danger-50)] rounded-lg p-3 mb-3 border border-[var(--color-danger-200)]">
        <p class="text-sm text-[var(--color-danger-800)]">{error}</p>
      </div>
    {/if}

    <!-- WHO CAN VIEW MY DATA -->
    <div class="mb-4">
      <h3 class="text-xs font-medium text-stone-400 dark:text-gray-500 uppercase tracking-wide mb-2">
        {$t('settings.data_sharing_who_can_view')}
      </h3>
      {#if grantsOut.length === 0}
        <p class="text-sm text-stone-400 dark:text-gray-500 italic">
          {$t('settings.data_sharing_no_grants_out')}
        </p>
      {:else}
        <div class="space-y-2">
          {#each grantsOut as grant (grant.id)}
            <div class="flex items-center gap-3 p-3 rounded-lg bg-stone-50 dark:bg-gray-800/50">
              <div
                class="w-8 h-8 rounded-full flex items-center justify-center text-white text-xs font-bold flex-shrink-0"
                style:background-color={profileColor(grant.grantee_name)}
              >
                {profileInitial(grant.grantee_name)}
              </div>
              <div class="flex-1 min-w-0">
                <p class="text-sm text-stone-700 dark:text-gray-200 font-medium truncate">
                  {grant.grantee_name}
                </p>
                <p class="text-xs text-stone-400 dark:text-gray-500">
                  {$t('settings.data_sharing_granted_at', { values: { date: formatDate(grant.granted_at) } })}
                </p>
              </div>
              <span class="text-xs font-medium px-2 py-0.5 rounded-full flex-shrink-0
                     {grant.access_level === 'full'
                       ? 'bg-[var(--color-success-50)] text-[var(--color-success)]'
                       : 'bg-[var(--color-info-50)] text-[var(--color-info)]'}">
                {grant.access_level === 'full'
                  ? $t('settings.data_sharing_access_full')
                  : $t('settings.data_sharing_access_read_only')}
              </span>
              {#if revokeTarget?.id === grant.id}
                <div class="flex gap-1">
                  <button
                    class="text-xs px-2 py-1 rounded bg-[var(--color-danger)] text-white min-h-[32px] disabled:opacity-50"
                    disabled={revoking}
                    onclick={() => handleRevoke(grant)}
                  >
                    {revoking ? '...' : $t('common.confirm')}
                  </button>
                  <button
                    class="text-xs px-2 py-1 rounded bg-stone-200 dark:bg-gray-700 text-stone-600 dark:text-gray-300 min-h-[32px]"
                    onclick={() => { revokeTarget = null; }}
                  >
                    {$t('common.cancel')}
                  </button>
                </div>
              {:else}
                <button
                  class="text-xs px-2.5 py-1 rounded-lg border border-[var(--color-danger-200)]
                         text-[var(--color-danger)] hover:bg-[var(--color-danger-50)] min-h-[32px] transition-colors"
                  onclick={() => { revokeTarget = grant; }}
                >
                  {$t('settings.data_sharing_revoke_btn')}
                </button>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <!-- DATA YOU CAN VIEW -->
    <div class="mb-4">
      <h3 class="text-xs font-medium text-stone-400 dark:text-gray-500 uppercase tracking-wide mb-2">
        {$t('settings.data_sharing_data_you_view')}
      </h3>
      {#if grantsIn.length === 0}
        <p class="text-sm text-stone-400 dark:text-gray-500 italic">
          {$t('settings.data_sharing_no_grants_in')}
        </p>
      {:else}
        <div class="space-y-2">
          {#each grantsIn as grant (grant.id)}
            <div class="flex items-center gap-3 p-3 rounded-lg bg-stone-50 dark:bg-gray-800/50">
              <div
                class="w-8 h-8 rounded-full flex items-center justify-center text-white text-xs font-bold flex-shrink-0"
                style:background-color={profileColor(grant.granter_name)}
              >
                {profileInitial(grant.granter_name)}
              </div>
              <div class="flex-1 min-w-0">
                <p class="text-sm text-stone-700 dark:text-gray-200 font-medium truncate">
                  {grant.granter_name}
                </p>
                <p class="text-xs text-stone-400 dark:text-gray-500">
                  {$t('settings.data_sharing_granted_by')}
                </p>
              </div>
              <span class="text-xs font-medium px-2 py-0.5 rounded-full flex-shrink-0
                     {grant.access_level === 'full'
                       ? 'bg-[var(--color-success-50)] text-[var(--color-success)]'
                       : 'bg-[var(--color-info-50)] text-[var(--color-info)]'}">
                {grant.access_level === 'full'
                  ? $t('settings.data_sharing_access_full')
                  : $t('settings.data_sharing_access_read_only')}
              </span>
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <!-- GRANT ACCESS (self-managed only) -->
    {#if profile.isSelfManaged && grantableProfiles.length > 0}
      <div class="border-t border-stone-100 dark:border-gray-800 pt-4">
        <h3 class="text-xs font-medium text-stone-400 dark:text-gray-500 uppercase tracking-wide mb-2">
          {$t('settings.data_sharing_grant_heading')}
        </h3>
        <div class="flex items-end gap-2">
          <div class="flex-1">
            <label for="grant-profile-select" class="sr-only">
              {$t('settings.data_sharing_select_profile')}
            </label>
            <select
              id="grant-profile-select"
              class="w-full px-3 py-2 rounded-lg border border-stone-200 dark:border-gray-700 text-sm
                     text-stone-700 dark:text-gray-200 bg-white dark:bg-gray-900 min-h-[40px]"
              bind:value={selectedProfileId}
            >
              <option value="">{$t('settings.data_sharing_select_profile')}</option>
              {#each grantableProfiles as p (p.id)}
                <option value={p.id}>{p.name}</option>
              {/each}
            </select>
          </div>
          <div>
            <label for="grant-access-level" class="sr-only">
              {$t('settings.data_sharing_heading')}
            </label>
            <select
              id="grant-access-level"
              class="px-3 py-2 rounded-lg border border-stone-200 dark:border-gray-700 text-sm
                     text-stone-700 dark:text-gray-200 bg-white dark:bg-gray-900 min-h-[40px]"
              bind:value={selectedAccessLevel}
            >
              <option value="read_only">{$t('settings.data_sharing_access_read_only')}</option>
              <option value="full">{$t('settings.data_sharing_access_full')}</option>
            </select>
          </div>
          <button
            class="px-4 py-2 rounded-lg bg-[var(--color-interactive)] text-white text-sm font-medium
                   min-h-[40px] disabled:opacity-50 transition-colors"
            disabled={!selectedProfileId || granting}
            onclick={handleGrant}
          >
            {granting ? '...' : $t('settings.data_sharing_grant_btn')}
          </button>
        </div>
      </div>
    {:else if !profile.isSelfManaged}
      <p class="text-xs text-stone-400 dark:text-gray-500 italic border-t border-stone-100 dark:border-gray-800 pt-3">
        {$t('settings.data_sharing_self_managed_only')}
      </p>
    {/if}
  {/if}
</section>
