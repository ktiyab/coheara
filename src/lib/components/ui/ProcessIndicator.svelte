<!--
  Spec 50 [NF-03]: Progressive process indicator.
  Shows stage-aware messages instead of generic "loading..." for long operations.
  Time-based stage advancement — simpler than event-based, no backend dependency.
-->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { onMount } from 'svelte';

  export interface ProcessStage {
    messageKey: string;
    percentHint: number;
    durationHint: number; // seconds before advancing to next stage
  }

  interface Props {
    stages: ProcessStage[];
    documentCount?: number;
  }

  let { stages, documentCount = 0 }: Props = $props();

  let currentStage = $state(0);
  let elapsedSeconds = $state(0);
  let startTime = Date.now();

  onMount(() => {
    const timer = setInterval(() => {
      elapsedSeconds = Math.floor((Date.now() - startTime) / 1000);

      // Advance to next stage when current stage duration exceeded
      let cumulative = 0;
      for (let i = 0; i < stages.length; i++) {
        cumulative += stages[i].durationHint;
        if (elapsedSeconds < cumulative) {
          currentStage = i;
          break;
        }
        if (i === stages.length - 1) {
          currentStage = i;
        }
      }
    }, 500);

    return () => clearInterval(timer);
  });

  let stageMessage = $derived(
    stages[currentStage]
      ? $t(stages[currentStage].messageKey, { values: { count: documentCount } })
      : ''
  );
  let progressPercent = $derived(stages[currentStage]?.percentHint ?? 0);
  let showPrivacyNote = $derived(elapsedSeconds > 15);
</script>

<div class="flex items-start gap-3" role="status" aria-live="polite">
  <!-- Animated pulse indicator -->
  <div
    class="shrink-0 w-8 h-8 rounded-full bg-[var(--color-primary-50)] flex items-center justify-center"
    aria-hidden="true"
  >
    <div class="w-3 h-3 rounded-full bg-[var(--color-primary)] animate-pulse"></div>
  </div>

  <div class="flex-1 min-w-0">
    <!-- Stage message -->
    <p class="text-sm font-medium text-[var(--color-text-primary)]">
      {stageMessage}
    </p>

    <!-- Progress bar -->
    <div class="w-full h-1 bg-[var(--color-surface)] rounded-full mt-2 overflow-hidden">
      <div
        class="h-full bg-[var(--color-primary)] rounded-full transition-all duration-1000 ease-out"
        style="width: {progressPercent}%"
      ></div>
    </div>

    <!-- Privacy/trust note for long waits -->
    {#if showPrivacyNote}
      <p class="text-xs text-[var(--color-text-muted)] mt-1.5">
        {$t('process.privacy_note')}
      </p>
    {/if}
  </div>
</div>
