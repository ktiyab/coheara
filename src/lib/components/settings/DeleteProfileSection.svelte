<!-- L5-01: Delete Profile — Danger zone with cryptographic erasure -->
<script lang="ts">
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
    if (confirmText !== 'DELETE MY DATA') {
      error = 'Please type "DELETE MY DATA" exactly';
      return;
    }
    if (!password) {
      error = 'Password required';
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

<section class="mt-8 border-t border-red-200 pt-6">
  <h2 class="text-sm font-medium text-red-600 mb-2">DANGER ZONE</h2>

  {#if !showConfirm}
    <button
      class="w-full px-4 py-3 bg-white border border-red-200 rounded-xl
             text-sm text-red-600 min-h-[44px]"
      onclick={() => (showConfirm = true)}
    >
      Delete profile and all data
    </button>
  {:else}
    <div class="bg-red-50 rounded-xl p-5 border border-red-200">
      <p class="text-sm text-red-800 mb-4">
        This will permanently delete all of <strong>{profile.name}'s</strong> health data.
        This cannot be undone.
      </p>

      <label for="delete-confirm" class="block text-sm text-red-700 mb-1">
        Type "DELETE MY DATA" to confirm:
      </label>
      <input
        id="delete-confirm"
        type="text"
        class="w-full px-4 py-3 rounded-lg border border-red-200 text-stone-700
               mb-3 min-h-[44px]"
        bind:value={confirmText}
        placeholder="DELETE MY DATA"
      />

      <label for="delete-password" class="block text-sm text-red-700 mb-1">Enter your password:</label>
      <input
        id="delete-password"
        type="password"
        class="w-full px-4 py-3 rounded-lg border border-red-200 text-stone-700
               mb-4 min-h-[44px]"
        bind:value={password}
      />

      {#if error}
        <p class="text-red-600 text-sm mb-3">{error}</p>
      {/if}

      <div class="flex gap-3">
        <button
          class="flex-1 px-4 py-3 bg-red-600 text-white rounded-xl text-sm
                 font-medium min-h-[44px] disabled:opacity-50"
          disabled={deleting || confirmText !== 'DELETE MY DATA' || !password}
          onclick={handleDelete}
        >
          {deleting ? 'Deleting...' : 'Delete everything'}
        </button>
        <button
          class="px-4 py-3 bg-white border border-stone-200 rounded-xl text-sm
                 text-stone-600 min-h-[44px]"
          onclick={() => {
            showConfirm = false;
            confirmText = '';
            password = '';
            error = null;
          }}
        >
          Cancel
        </button>
      </div>
    </div>
  {/if}
</section>
