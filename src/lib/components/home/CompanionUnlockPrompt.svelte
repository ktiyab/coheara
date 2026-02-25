<!-- MP-02: Companion Unlock Prompt — Android Multi-User setup pattern.
     Non-blocking card in Home Zone B prompting caregivers to unlock
     managed profiles for companion phone access. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { unlockForCompanion, type CompanionProfileInfo } from '$lib/api/companion';
  import { PROFILE_COLORS, type ProfileInfo } from '$lib/types/profile';
  import { SupervisorIcon } from '$lib/components/icons/md';

  interface Props {
    /** Managed profiles not yet unlocked for companion. */
    managedProfiles: ProfileInfo[];
    onDismiss: () => void;
    onAllDone: () => void;
  }
  let { managedProfiles, onDismiss, onAllDone }: Props = $props();

  /** Track unlock state per profile. */
  let unlockStates = $state<Record<string, 'idle' | 'unlocking' | 'done' | 'error'>>({});
  let passwords = $state<Record<string, string>>({});
  let errors = $state<Record<string, string>>({});

  let unlockedCount = $derived(
    Object.values(unlockStates).filter((s) => s === 'done').length
  );

  function profileColor(p: ProfileInfo): string {
    return p.color_index != null
      ? PROFILE_COLORS[p.color_index % PROFILE_COLORS.length]
      : PROFILE_COLORS[p.name.charCodeAt(0) % PROFILE_COLORS.length];
  }

  function profileInitial(name: string): string {
    return (name || 'P').charAt(0).toUpperCase();
  }

  async function handleUnlock(p: ProfileInfo) {
    const pw = passwords[p.id] ?? '';
    if (!pw) return;

    unlockStates[p.id] = 'unlocking';
    errors[p.id] = '';

    try {
      await unlockForCompanion(p.id, pw);
      unlockStates[p.id] = 'done';
      passwords[p.id] = '';
    } catch (e) {
      unlockStates[p.id] = 'error';
      errors[p.id] = e instanceof Error ? e.message : String(e);
    }
  }

  function handleDone() {
    if (unlockedCount > 0) {
      onAllDone();
    } else {
      onDismiss();
    }
  }
</script>

<section class="mx-[var(--spacing-page-x)] mt-3">
  <div class="bg-white dark:bg-gray-900 rounded-xl p-5 border border-stone-100 dark:border-gray-800 shadow-sm">
    <div class="flex items-center gap-3 mb-3">
      <div class="w-10 h-10 rounded-lg bg-[var(--color-success)] flex items-center justify-center flex-shrink-0">
        <SupervisorIcon class="w-5 h-5 text-white" />
      </div>
      <h2 class="text-sm font-medium text-stone-700 dark:text-gray-200">
        {$t('companion.unlock_heading')}
      </h2>
    </div>
    <p class="text-xs text-stone-400 dark:text-gray-500 mb-4">
      {$t('companion.unlock_description')}
    </p>

    <div class="space-y-3">
      {#each managedProfiles as p (p.id)}
        {@const state = unlockStates[p.id] ?? 'idle'}
        <div class="flex items-center gap-3">
          <!-- Avatar -->
          <div
            class="w-8 h-8 rounded-full flex items-center justify-center text-white text-xs font-bold flex-shrink-0"
            style:background-color={profileColor(p)}
          >
            {profileInitial(p.name)}
          </div>

          {#if state === 'done'}
            <!-- Unlocked -->
            <div class="flex-1 flex items-center gap-2">
              <span class="text-sm text-stone-700 dark:text-gray-200">{p.name}</span>
              <span class="text-xs text-[var(--color-success)] font-medium">
                {$t('companion.unlock_success')}
              </span>
            </div>
          {:else}
            <!-- Password input + unlock button -->
            <div class="flex-1 flex items-center gap-2">
              <span class="text-sm text-stone-700 dark:text-gray-200 w-20 truncate flex-shrink-0">{p.name}</span>
              <input
                type="password"
                class="flex-1 px-3 py-1.5 rounded-lg border text-sm min-h-[36px]
                       dark:bg-gray-800
                       {state === 'error'
                         ? 'border-[var(--color-danger-200)] text-[var(--color-danger)]'
                         : 'border-stone-200 dark:border-gray-700 text-stone-700 dark:text-gray-200'}"
                placeholder={$t('companion.unlock_password_placeholder')}
                aria-label={$t('companion.unlock_password_label', { values: { name: p.name } })}
                bind:value={passwords[p.id]}
                onkeydown={(e) => { if (e.key === 'Enter') handleUnlock(p); }}
              />
              <button
                class="px-3 py-1.5 rounded-lg text-xs font-medium min-h-[36px] transition-colors
                       {state === 'unlocking'
                         ? 'bg-stone-100 dark:bg-gray-800 text-stone-400 cursor-wait'
                         : 'bg-[var(--color-success)] text-white hover:opacity-90'}
                       disabled:opacity-50"
                disabled={state === 'unlocking' || !(passwords[p.id]?.trim())}
                onclick={() => handleUnlock(p)}
              >
                {state === 'unlocking' ? '...' : $t('companion.unlock_btn')}
              </button>
            </div>
          {/if}
        </div>
        {#if errors[p.id]}
          <p class="text-xs text-[var(--color-danger)] ml-11">{errors[p.id]}</p>
        {/if}
      {/each}
    </div>

    <!-- Actions -->
    <div class="flex justify-end gap-2 mt-4 pt-3 border-t border-stone-100 dark:border-gray-800">
      <button
        class="px-4 py-2 text-xs text-stone-500 dark:text-gray-400 hover:text-stone-700 dark:hover:text-gray-200
               min-h-[36px] rounded-lg transition-colors"
        onclick={onDismiss}
      >
        {$t('companion.unlock_not_now')}
      </button>
      {#if unlockedCount > 0}
        <button
          class="px-4 py-2 text-xs font-medium bg-[var(--color-success)] text-white
                 rounded-lg min-h-[36px] transition-colors"
          onclick={handleDone}
        >
          {$t('companion.unlock_done')}
        </button>
      {/if}
    </div>
  </div>
</section>
