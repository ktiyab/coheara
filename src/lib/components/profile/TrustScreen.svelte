<!-- V1: Landing page with logo, value proposition, and language picker -->
<script lang="ts">
  import { t, locale } from 'svelte-i18n';
  import { setUserPreference } from '$lib/api/ai';
  import CohearaLogo from '$lib/components/ui/CohearaLogo.svelte';
  import { CheckOutline } from 'flowbite-svelte-icons';

  interface Props {
    onContinue: () => void;
  }
  let { onContinue }: Props = $props();

  const languages = [
    { code: 'en', label: 'English' },
    { code: 'fr', label: 'Fran\u00e7ais' },
    { code: 'de', label: 'Deutsch' },
  ];

  const benefits = [
    'trust.benefit_aggregate',
    'trust.benefit_intelligence',
    'trust.benefit_safety',
    'trust.benefit_portability',
    'trust.benefit_privacy',
  ] as const;

  async function selectLanguage(code: string) {
    locale.set(code);
    try {
      await setUserPreference('language', code);
    } catch {
      // Best effort: preference saved once profile exists
    }
  }
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-6 max-w-2xl mx-auto">
  <!-- Logo -->
  <CohearaLogo class="w-80 h-80 text-[var(--color-interactive)]" />

  <!-- Language selector -->
  <div class="flex gap-2">
    {#each languages as lang}
      <button
        class="px-3 py-1.5 rounded-full text-sm font-medium transition-colors min-h-[44px]
               {$locale === lang.code
                 ? 'bg-[var(--color-interactive)] text-white'
                 : 'bg-stone-100 dark:bg-gray-800 text-stone-600 dark:text-gray-300 hover:bg-stone-200 dark:hover:bg-gray-700'}"
        onclick={() => selectLanguage(lang.code)}
      >
        {lang.label}
      </button>
    {/each}
  </div>

  <!-- Hero title -->
  <h1 class="text-2xl font-bold text-stone-900 dark:text-gray-50">{$t('trust.heading')}</h1>

  <!-- Tagline -->
  <p class="text-lg text-stone-600 dark:text-gray-300 text-center leading-relaxed">
    {$t('trust.value_proposition')}
  </p>

  <!-- Value propositions -->
  <div class="flex flex-col gap-4 text-stone-700 dark:text-gray-300 text-base">
    {#each benefits as key}
      <div class="flex items-start gap-3">
        <CheckOutline class="w-4 h-4 text-[var(--color-success)] mt-1 shrink-0" />
        <p>{$t(key)}</p>
      </div>
    {/each}
  </div>

  <!-- Primary CTA -->
  <div class="mt-4">
    <button
      class="px-6 py-3 min-h-[48px] text-base font-medium rounded-lg text-white transition-colors cursor-pointer
             bg-[var(--color-interactive)] hover:bg-[var(--color-interactive-hover)] active:bg-[var(--color-interactive-active)]
             focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-interactive)]"
      onclick={onContinue}
    >
      {$t('trust.cta')}
    </button>
  </div>
</div>
