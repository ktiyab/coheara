<script lang="ts">
  import { createProfile } from '$lib/api/profile';
  import type { ProfileCreateResult } from '$lib/types/profile';

  interface Props {
    onCreated: (result: ProfileCreateResult) => void;
    onError: (error: string) => void;
  }
  let { onCreated, onError }: Props = $props();

  let name = $state('');
  let password = $state('');
  let confirmPassword = $state('');
  let caregiverMode = $state(false);
  let caregiverName = $state('');
  let loading = $state(false);
  let passwordError = $state('');

  function validatePassword(): boolean {
    if (password.length < 6) {
      passwordError = 'Password must be at least 6 characters';
      return false;
    }
    if (password !== confirmPassword) {
      passwordError = 'Passwords do not match';
      return false;
    }
    passwordError = '';
    return true;
  }

  async function handleCreate() {
    if (!name.trim()) return;
    if (!validatePassword()) return;

    loading = true;
    try {
      const result = await createProfile(
        name.trim(),
        password,
        caregiverMode ? caregiverName.trim() : null,
      );
      onCreated(result);
    } catch (e) {
      onError(String(e));
    } finally {
      loading = false;
    }
  }
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-6 max-w-md mx-auto">
  <h2 class="text-2xl font-bold text-stone-800">Create your profile</h2>

  <div class="w-full flex flex-col gap-4">
    <label class="flex flex-col gap-1">
      <span class="text-stone-600 text-sm font-medium">What's your name?</span>
      <input
        type="text"
        bind:value={name}
        placeholder="Marie"
        class="px-4 py-3 rounded-lg border border-stone-300 text-lg min-h-[44px]
               focus:border-[var(--color-primary)] focus:outline-none"
        autocomplete="off"
      />
    </label>

    <div class="flex items-center gap-2">
      <input type="checkbox" id="caregiver" bind:checked={caregiverMode}
             class="min-h-[44px] min-w-[44px]" />
      <label for="caregiver" class="text-stone-600 text-sm">
        I'm setting this up for someone I care for
      </label>
    </div>

    {#if caregiverMode}
      <label class="flex flex-col gap-1">
        <span class="text-stone-600 text-sm font-medium">Your name (caregiver)</span>
        <input
          type="text"
          bind:value={caregiverName}
          placeholder="Sophie"
          class="px-4 py-3 rounded-lg border border-stone-300 text-lg min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
        />
      </label>
    {/if}

    <label class="flex flex-col gap-1">
      <span class="text-stone-600 text-sm font-medium">Create a password</span>
      <input
        type="password"
        bind:value={password}
        placeholder="At least 6 characters"
        class="px-4 py-3 rounded-lg border border-stone-300 text-lg min-h-[44px]
               focus:border-[var(--color-primary)] focus:outline-none"
        autocomplete="new-password"
      />
    </label>

    <label class="flex flex-col gap-1">
      <span class="text-stone-600 text-sm font-medium">Confirm password</span>
      <input
        type="password"
        bind:value={confirmPassword}
        placeholder="Type it again"
        class="px-4 py-3 rounded-lg border border-stone-300 text-lg min-h-[44px]
               focus:border-[var(--color-primary)] focus:outline-none"
        autocomplete="new-password"
      />
    </label>

    {#if passwordError}
      <p class="text-red-600 text-sm">{passwordError}</p>
    {/if}

    <button
      class="mt-2 px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-lg
             font-medium hover:brightness-110 disabled:opacity-50 min-h-[44px]"
      onclick={handleCreate}
      disabled={loading || !name.trim() || !password}
    >
      {loading ? 'Creating...' : 'Create profile'}
    </button>
  </div>
</div>
