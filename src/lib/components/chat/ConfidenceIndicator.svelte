<script lang="ts">
  interface Props {
    confidence: number;
  }
  let { confidence }: Props = $props();

  let display = $derived.by(() => {
    if (confidence >= 0.8) {
      return { label: 'Well supported', color: 'text-green-600', icon: '\u25CF' };
    } else if (confidence >= 0.5) {
      return { label: 'Partially supported', color: 'text-amber-600', icon: '\u25D0' };
    } else {
      return { label: 'Limited information', color: 'text-stone-400', icon: '\u25CB' };
    }
  });
</script>

<div
  class="flex items-center gap-1.5 text-xs {display.color}"
  title="This indicates how much of this answer is directly supported by your documents."
  role="status"
  aria-label="Confidence: {display.label}"
>
  <span aria-hidden="true">{display.icon}</span>
  <span>{display.label}</span>
</div>
