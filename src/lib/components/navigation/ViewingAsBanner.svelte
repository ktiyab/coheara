<!-- MP-02 §11.2: "Viewing As" banner — AWS IAM + Epic MyChart pattern.
     Prominent amber bar when operating on a managed profile's data.
     Medical safety: users must ALWAYS know whose health data they're seeing. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { ChevronLeftIcon } from '$lib/components/icons/md';

  interface Props {
    managedName: string;
    onReturnToSelf: () => void;
  }
  let { managedName, onReturnToSelf }: Props = $props();
</script>

<div
  class="h-10 w-full bg-amber-50 dark:bg-amber-900/20 border-b border-amber-200 dark:border-amber-800
         px-6 flex items-center justify-between flex-shrink-0"
  role="status"
  aria-live="polite"
>
  <span class="text-sm text-amber-800 dark:text-amber-400 font-medium truncate">
    {$t('nav.viewing_as_banner', { values: { name: managedName } })}
  </span>
  <button
    class="flex items-center gap-1.5 text-xs font-medium text-amber-700 dark:text-amber-400
           hover:text-amber-900 dark:hover:text-amber-300 transition-colors min-h-[32px] px-2
           rounded-md hover:bg-amber-100 dark:hover:bg-amber-900/30"
    onclick={onReturnToSelf}
  >
    <ChevronLeftIcon class="w-3.5 h-3.5" />
    {$t('nav.return_to_self')}
  </button>
</div>
