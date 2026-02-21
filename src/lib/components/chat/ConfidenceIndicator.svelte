<script lang="ts">
  import { t } from 'svelte-i18n';

  interface Props {
    confidence: number;
  }
  let { confidence }: Props = $props();

  let display = $derived.by(() => {
    if (confidence >= 0.8) {
      return { label: $t('chat.confidence_well_supported'), color: 'text-[var(--color-success)]', icon: '\u25CF' };
    } else if (confidence >= 0.5) {
      return { label: $t('chat.confidence_partially_supported'), color: 'text-[var(--color-warning)]', icon: '\u25D0' };
    } else {
      return { label: $t('chat.confidence_limited_info'), color: 'text-stone-500 dark:text-gray-400', icon: '\u25CB' };
    }
  });
</script>

<div
  class="flex items-center gap-1.5 text-xs {display.color}"
  title={$t('chat.confidence_tooltip')}
  role="status"
  aria-label={$t('chat.confidence_aria', { values: { label: display.label } })}
>
  <span aria-hidden="true">{display.icon}</span>
  <span>{display.label}</span>
</div>
