<!-- I18N-38: First-launch trust screen with language picker -->
<script lang="ts">
  import { t, locale } from 'svelte-i18n';
  import { setUserPreference } from '$lib/api/ai';
  import Button from '$lib/components/ui/Button.svelte';

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
                 ? 'bg-[var(--color-interactive)] text-white'
                 : 'bg-stone-100 text-stone-600 hover:bg-stone-200'}"
        onclick={() => selectLanguage(lang.code)}
      >
        {lang.label}
      </button>
    {/each}
  </div>

  <h1 class="text-3xl font-bold text-stone-800">{$t('trust.heading')}</h1>

  <!-- Spec 45 [ON-02]: Value proposition BEFORE privacy badges -->
  <p class="text-lg text-stone-600 text-center leading-relaxed">
    {$t('trust.value_proposition')}
  </p>

  <hr class="w-full border-stone-200" />

  <div class="flex flex-col gap-4 text-stone-600 text-base">
    <div class="flex items-start gap-3">
      <span class="text-[var(--color-success)] mt-1">&#x2713;</span>
      <p>{$t('trust.privacy_local')}</p>
    </div>
    <div class="flex items-start gap-3">
      <span class="text-[var(--color-success)] mt-1">&#x2713;</span>
      <p>{$t('trust.privacy_encrypted')}</p>
    </div>
    <div class="flex items-start gap-3">
      <span class="text-[var(--color-success)] mt-1">&#x2713;</span>
      <p>{$t('trust.privacy_ai')}</p>
    </div>
  </div>

  <div class="mt-4">
    <Button variant="primary" size="lg" onclick={onContinue}>
      {$t('trust.get_started')}
    </Button>
  </div>
</div>
