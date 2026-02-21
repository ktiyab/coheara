<!-- Spec 45 [ON-02]: First-class profile type choice before CreateProfile -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { PersonIcon, GroupIcon } from '$lib/components/icons/md';

  interface Props {
    onSelect: (isCaregiver: boolean) => void;
  }
  let { onSelect }: Props = $props();
</script>

<div class="flex flex-col items-center px-8 gap-6 max-w-md mx-auto">
  <h2 class="text-2xl font-bold text-stone-800 dark:text-gray-100">{$t('profile.type_choice_heading')}</h2>

  <div class="w-full flex flex-col gap-4">
    <button
      class="group w-full text-left p-5 rounded-xl border-2 border-stone-200 dark:border-gray-700
             hover:border-[var(--color-interactive)] hover:bg-[var(--color-interactive-50)]
             transition-colors min-h-[44px] cursor-pointer"
      onclick={() => onSelect(false)}
    >
      <div class="flex items-center gap-4">
        <PersonIcon class="w-12 h-12 flex-shrink-0 text-stone-500 dark:text-gray-400 transition-colors group-hover:text-[var(--color-interactive)]" />
        <div>
          <p class="font-semibold text-stone-800 dark:text-gray-200">{$t('profile.type_self')}</p>
          <p class="text-sm text-stone-500 dark:text-gray-400 mt-1">{$t('profile.type_self_desc')}</p>
        </div>
      </div>
    </button>

    <button
      class="group w-full text-left p-5 rounded-xl border-2 border-stone-200 dark:border-gray-700
             hover:border-[var(--color-interactive)] hover:bg-[var(--color-interactive-50)]
             transition-colors min-h-[44px] cursor-pointer"
      onclick={() => onSelect(true)}
    >
      <div class="flex items-center gap-4">
        <GroupIcon class="w-12 h-12 flex-shrink-0 text-stone-500 dark:text-gray-400 transition-colors group-hover:text-[var(--color-interactive)]" />
        <div>
          <p class="font-semibold text-stone-800 dark:text-gray-200">{$t('profile.type_caregiver')}</p>
          <p class="text-sm text-stone-500 dark:text-gray-400 mt-1">{$t('profile.type_caregiver_desc')}</p>
        </div>
      </div>
    </button>
  </div>
</div>
