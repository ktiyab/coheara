<!-- I18N-38 + V16: Language selector — inline pill group, endonym labels. -->
<script lang="ts">
  import { locale, t } from 'svelte-i18n';
  import { setUserPreference } from '$lib/api/ai';

  const languages = [
    { code: 'en', label: 'English' },
    { code: 'fr', label: 'Fran\u00e7ais' },
    { code: 'de', label: 'Deutsch' },
  ];

  async function changeLanguage(code: string) {
    locale.set(code);
    try {
      await setUserPreference('language', code);
    } catch (e) {
      console.error('Failed to save language preference:', e);
    }
  }
</script>

<div class="flex gap-1 rounded-lg bg-stone-100 dark:bg-gray-800 p-0.5" role="radiogroup" aria-label={$t('settings.language_label')}>
  {#each languages as lang (lang.code)}
    <button
      role="radio"
      aria-checked={$locale === lang.code}
      class="flex-1 px-3 py-2 rounded-md text-sm font-medium min-h-[44px] transition-colors
             focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-[var(--color-interactive)]
             {$locale === lang.code
               ? 'bg-white dark:bg-gray-700 text-stone-800 dark:text-gray-100 shadow-sm'
               : 'text-stone-500 dark:text-gray-400 hover:text-stone-700 dark:hover:text-gray-200'}"
      onclick={() => changeLanguage(lang.code)}
    >
      {lang.label}
    </button>
  {/each}
</div>
