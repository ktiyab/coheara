<!-- L3-04: Low-confidence field flag with patient-friendly message. -->
<script lang="ts">
  interface Props {
    confidence: number;
    fieldLabel: string;
  }
  let { confidence, fieldLabel }: Props = $props();

  let severityClass = $derived(
    confidence < 0.50
      ? 'bg-red-50 border-red-200 text-red-800'
      : 'bg-amber-50 border-amber-200 text-amber-800'
  );

  let explanationText = $derived(
    confidence < 0.50
      ? 'This field was extracted from very low-quality text. The original might say something quite different.'
      : 'This field was extracted from a low-quality image. The original might say something different.'
  );
</script>

<div class="rounded-lg border px-3 py-2 {severityClass}" role="alert">
  <p class="text-sm font-medium">
    I'm not sure I read this correctly -- please check
  </p>
  <p class="text-xs mt-1 opacity-80">
    {explanationText}
  </p>
  <p class="text-xs mt-1 opacity-60">
    Confidence: {Math.round(confidence * 100)}%
  </p>
</div>
