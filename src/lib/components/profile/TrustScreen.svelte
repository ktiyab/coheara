<!-- I18N-38: First-launch trust screen with language picker -->
<script lang="ts">
  import { t, locale } from 'svelte-i18n';
  import { setUserPreference } from '$lib/api/ai';

  interface Props {
    onContinue: () => void;
  }
  let { onContinue }: Props = $props();

  const languages = [
    { code: 'en', label: 'English' },
    { code: 'fr', label: 'Fran\u00e7ais' },
    { code: 'de', label: 'Deutsch' },
  ];

  async function selectLanguage(code: string) {
    locale.set(code);
    try {
      await setUserPreference('language', code);
    } catch {
      // Best effort — preference will be saved once profile exists
    }
  }
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-8 max-w-lg mx-auto">
  <!-- I18N-38: Language selector at first launch -->
  <div class="flex gap-2">
    {#each languages as lang}
      <button
        class="px-3 py-1.5 rounded-full text-sm font-medium transition-colors min-h-[44px]
               {$locale === lang.code
                 ? 'bg-teal-600 text-white'
                 : 'bg-stone-100 text-stone-600 hover:bg-stone-200'}"
        onclick={() => selectLanguage(lang.code)}
      >
        {lang.label}
      </button>
    {/each}
  </div>

  <h1 class="text-3xl font-bold text-stone-800">Welcome to Coheara</h1>
  <p class="text-lg text-stone-600 text-center leading-relaxed">
    Your personal medical document assistant.
  </p>

  <div class="flex flex-col gap-4 text-stone-600 text-base">
    <div class="flex items-start gap-3">
      <span class="text-green-600 mt-1">&#x2713;</span>
      <p>{@html $t('trust.benefit_local')}</p>
    </div>
    <div class="flex items-start gap-3">
      <span class="text-green-600 mt-1">&#x2713;</span>
      <p>{@html $t('trust.benefit_encrypted')}</p>
    </div>
    <div class="flex items-start gap-3">
      <span class="text-green-600 mt-1">&#x2713;</span>
      <p>{$t('trust.benefit_private')}</p>
    </div>
  </div>

  <button
    class="mt-4 px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-lg font-medium
           hover:brightness-110 focus-visible:outline focus-visible:outline-2
           focus-visible:outline-offset-2 focus-visible:outline-[var(--color-primary)]
           min-h-[44px] min-w-[44px]"
    onclick={onContinue}
  >
    {$t('trust.continue')}
  </button>
</div>
