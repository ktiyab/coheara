<!-- Spec 45 [ON-02]: First-class profile type choice before CreateProfile -->
<script lang="ts">
  import { t } from 'svelte-i18n';

  interface Props {
    onSelect: (isCaregiver: boolean) => void;
  }
  let { onSelect }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-8 max-w-md mx-auto">
  <h2 class="text-2xl font-bold text-stone-800">{$t('profile.type_choice_heading')}</h2>

  <div class="w-full flex flex-col gap-4">
    <button
      class="w-full text-left p-5 rounded-xl border-2 border-stone-200
             hover:border-[var(--color-primary)] hover:bg-[var(--color-primary-50)]
             transition-colors min-h-[44px] cursor-pointer"
      onclick={() => onSelect(false)}
    >
      <div class="flex items-start gap-4">
        <span class="text-2xl flex-shrink-0 mt-0.5" aria-hidden="true">&#x1F464;</span>
        <div>
          <p class="font-semibold text-stone-800">{$t('profile.type_self')}</p>
          <p class="text-sm text-stone-500 mt-1">{$t('profile.type_self_desc')}</p>
        </div>
      </div>
    </button>

    <button
      class="w-full text-left p-5 rounded-xl border-2 border-stone-200
             hover:border-[var(--color-primary)] hover:bg-[var(--color-primary-50)]
             transition-colors min-h-[44px] cursor-pointer"
      onclick={() => onSelect(true)}
    >
      <div class="flex items-start gap-4">
        <span class="text-2xl flex-shrink-0 mt-0.5" aria-hidden="true">&#x1F465;</span>
        <div>
          <p class="font-semibold text-stone-800">{$t('profile.type_caregiver')}</p>
          <p class="text-sm text-stone-500 mt-1">{$t('profile.type_caregiver_desc')}</p>
        </div>
      </div>
    </button>
  </div>
</div>
