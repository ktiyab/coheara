<!-- L3-04: Low-confidence field flag with patient-friendly message. -->
<script lang="ts">
  import { t } from 'svelte-i18n';

  interface Props {
    confidence: number;
    fieldLabel: string;
  }
  let { confidence, fieldLabel }: Props = $props();

  let severityClass = $derived(
    confidence < 0.50
      ? 'bg-[var(--color-danger-50)] border-[var(--color-danger-200)] text-[var(--color-danger-800)]'
      : 'bg-[var(--color-warning-50)] border-[var(--color-warning-200)] text-[var(--color-warning-800)]'
  );

  let explanationText = $derived(
    confidence < 0.50
      ? $t('review.confidence_very_low')
      : $t('review.confidence_low')
  );
</script>

<div class="rounded-lg border px-3 py-2 {severityClass}" role="alert">
  <p class="text-sm font-medium">
    {$t('review.confidence_check')}
  </p>
  <p class="text-xs mt-1 opacity-80">
    {explanationText}
  </p>
  <p class="text-xs mt-1 opacity-60">
    {$t('review.confidence_label')} {Math.round(confidence * 100)}%
  </p>
</div>
