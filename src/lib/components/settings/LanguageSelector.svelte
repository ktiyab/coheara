<!-- I18N-38 + 6.7: Language selector with endonym labels. -->
<script lang="ts">
  import { locale } from 'svelte-i18n';
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

<fieldset class="space-y-1">
  <legend class="text-sm font-medium text-stone-500 mb-2">Language</legend>
  {#each languages as lang}
    <label class="flex items-center gap-3 py-2 min-h-[44px] cursor-pointer">
      <input
        type="radio"
        name="language"
        value={lang.code}
        checked={$locale === lang.code}
        onchange={() => changeLanguage(lang.code)}
        class="w-4 h-4 text-teal-600 focus:ring-teal-500"
      />
      <span class="text-sm text-stone-700">{lang.label}</span>
    </label>
  {/each}
</fieldset>
