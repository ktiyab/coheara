<script lang="ts">
  import type { ProfileInfo } from '$lib/types/profile';

  interface Props {
    profiles: ProfileInfo[];
    onSelect: (profile: ProfileInfo) => void;
    onCreateNew: () => void;
  }
  let { profiles, onSelect, onCreateNew }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-8 max-w-md mx-auto">
  <h2 class="text-2xl font-bold text-stone-800">Who's using Coheara?</h2>

  <div class="flex flex-col gap-3 w-full">
    {#each profiles as profile}
      <button
        class="w-full flex items-center gap-4 p-4 bg-white rounded-xl border border-stone-200
               hover:border-[var(--color-primary)] hover:shadow-sm transition-all
               min-h-[44px] text-left"
        onclick={() => onSelect(profile)}
      >
        <div class="w-12 h-12 rounded-full bg-stone-200 flex items-center justify-center
                    text-stone-600 text-xl font-bold">
          {profile.name.charAt(0).toUpperCase()}
        </div>
        <div class="flex flex-col">
          <span class="text-stone-800 font-medium text-lg">{profile.name}</span>
          {#if profile.managed_by}
            <span class="text-stone-400 text-sm">Managed by {profile.managed_by}</span>
          {/if}
        </div>
      </button>
    {/each}
  </div>

  <button
    class="px-6 py-3 border border-dashed border-stone-300 rounded-xl text-stone-500
           hover:border-[var(--color-primary)] hover:text-[var(--color-primary)]
           transition-all min-h-[44px]"
    onclick={onCreateNew}
  >
    + Create new profile
  </button>
</div>
