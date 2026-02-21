<!-- L5-01: Delete Profile — Danger zone with cryptographic erasure -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { eraseProfile } from '$lib/api/trust';

  import { profile } from '$lib/stores/profile.svelte';

  interface Props {
    onDeleted: () => void;
  }
  let { onDeleted }: Props = $props();

  let showConfirm = $state(false);
  let confirmText = $state('');
  let password = $state('');
  let deleting = $state(false);
  let error: string | null = $state(null);

  async function handleDelete() {
    if (confirmText !== $t('delete_profile.confirm_phrase')) {
      error = $t('delete_profile.confirm_mismatch', { values: { phrase: $t('delete_profile.confirm_phrase') } });
      return;
    }
    if (!password) {
      error = $t('delete_profile.password_required');
      return;
    }

    deleting = true;
    error = null;
    try {
      await eraseProfile({
        profile_id: '',
        confirmation_text: confirmText,
        password,
      });
      onDeleted();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      deleting = false;
    }
  }
</script>

<section class="mt-8 border-t border-[var(--color-danger-200)] pt-6">
  <h2 class="text-sm font-medium text-[var(--color-danger)] mb-2">{$t('delete_profile.danger_zone')}</h2>

  {#if !showConfirm}
    <button
      class="w-full px-4 py-3 bg-white dark:bg-gray-900 border border-[var(--color-danger-200)] rounded-xl
             text-sm text-[var(--color-danger)] min-h-[44px]"
      onclick={() => (showConfirm = true)}
    >
      {$t('delete_profile.delete_button')}
    </button>
  {:else}
    <div class="bg-[var(--color-danger-50)] rounded-xl p-5 border border-[var(--color-danger-200)]">
      <p class="text-sm text-[var(--color-danger-800)] mb-4">
        {$t('delete_profile.permanent_warning', { values: { name: profile.name } })}
        {$t('delete_profile.cannot_undo')}
      </p>

      <label for="delete-confirm" class="block text-sm text-[var(--color-danger)] mb-1">
        {$t('delete_profile.type_confirm_label', { values: { phrase: $t('delete_profile.confirm_phrase') } })}
      </label>
      <input
        id="delete-confirm"
        type="text"
        class="w-full px-4 py-3 rounded-lg border border-[var(--color-danger-200)] text-stone-700 dark:text-gray-200
               mb-3 min-h-[44px]"
        bind:value={confirmText}
        placeholder={$t('delete_profile.confirm_placeholder')}
      />

      <label for="delete-password" class="block text-sm text-[var(--color-danger)] mb-1">{$t('delete_profile.password_label')}</label>
      <input
        id="delete-password"
        type="password"
        class="w-full px-4 py-3 rounded-lg border border-[var(--color-danger-200)] text-stone-700 dark:text-gray-200
               mb-4 min-h-[44px]"
        bind:value={password}
      />

      {#if error}
        <p class="text-[var(--color-danger)] text-sm mb-3">{error}</p>
      {/if}

      <div class="flex gap-3">
        <button
          class="flex-1 px-4 py-3 bg-[var(--color-danger)] text-white rounded-xl text-sm
                 font-medium min-h-[44px] disabled:opacity-50"
          disabled={deleting || confirmText !== $t('delete_profile.confirm_phrase') || !password}
          onclick={handleDelete}
        >
          {deleting ? $t('delete_profile.deleting') : $t('delete_profile.delete_everything')}
        </button>
        <button
          class="px-4 py-3 bg-white dark:bg-gray-900 border border-stone-200 dark:border-gray-700 rounded-xl text-sm
                 text-stone-600 dark:text-gray-300 min-h-[44px]"
          onclick={() => {
            showConfirm = false;
            confirmText = '';
            password = '';
            error = null;
          }}
        >
          {$t('common.cancel')}
        </button>
      </div>
    </div>
  {/if}
</section>
