<!-- REVIEW-01: Date grouping separator (Signal/Apple Messages pattern). -->
<script lang="ts">
  import { t, locale } from 'svelte-i18n';

  let { date }: { date: string } = $props();

  let label = $derived.by(() => {
    const d = new Date(date);
    const now = new Date();
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const target = new Date(d.getFullYear(), d.getMonth(), d.getDate());
    const diff = Math.floor((today.getTime() - target.getTime()) / 86_400_000);

    if (diff === 0) return $t('chat.date_today');
    if (diff === 1) return $t('chat.date_yesterday');

    return d.toLocaleDateString($locale ?? 'en', {
      weekday: 'long',
      month: 'long',
      day: 'numeric',
    });
  });
</script>

<div class="flex items-center justify-center py-2" role="separator" aria-label={label}>
  <span
    class="text-xs px-3 py-1 rounded-full
      bg-stone-100 dark:bg-gray-800 text-stone-500 dark:text-gray-400"
  >
    {label}
  </span>
</div>
