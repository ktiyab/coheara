<!-- LP-07: Check-in nudge card — prompts user to record how they feel. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { CloseIcon, SearchIcon } from '$lib/components/icons/md';

  interface NudgeData {
    should_nudge: boolean;
    nudge_type: string | null;
    message: string | null;
    related_medication: string | null;
  }

  interface Props {
    nudge: NudgeData;
  }

  let { nudge }: Props = $props();
  let dismissed = $state(false);

  function handleTap() {
    const prefill = nudge.nudge_type === 'PostMedicationChange' && nudge.related_medication
      ? $t('home.nudge_medication_prefill', { values: { med: nudge.related_medication } })
      : $t('chat.prefill_symptom');
    navigation.navigate('chat', { prefill });
  }
</script>

{#if nudge.should_nudge && !dismissed}
  <section class="px-6 py-3" aria-label={$t('home.nudge_aria')}>
    <div
      class="w-full flex items-center gap-3 p-3 rounded-xl bg-[var(--color-primary-50)] dark:bg-blue-900/20
             border border-[var(--color-primary-200)] dark:border-blue-800"
    >
      <button
        onclick={handleTap}
        class="flex items-center gap-3 flex-1 min-w-0 text-left cursor-pointer
               hover:opacity-80 transition-opacity"
      >
        <div class="shrink-0 w-10 h-10 rounded-lg bg-[var(--color-primary-100)] dark:bg-blue-800/40 flex items-center justify-center">
          <SearchIcon class="w-5 h-5 text-[var(--color-primary)]" />
        </div>
        <div class="flex-1 min-w-0">
          <p class="text-sm font-medium text-[var(--color-text-primary)]">
            {nudge.nudge_type === 'PostMedicationChange'
              ? $t('home.nudge_medication_title')
              : $t('home.nudge_checkin_title')}
          </p>
          <p class="text-xs text-[var(--color-text-muted)] line-clamp-2">
            {nudge.message ?? $t('home.nudge_checkin_default')}
          </p>
        </div>
      </button>
      <button
        onclick={() => { dismissed = true; }}
        class="shrink-0 p-1.5 rounded-lg hover:bg-stone-200 dark:hover:bg-gray-700 transition-colors"
        aria-label={$t('home.nudge_dismiss_aria')}
      >
        <CloseIcon class="w-4 h-4 text-[var(--color-text-muted)]" />
      </button>
    </div>
  </section>
{/if}
