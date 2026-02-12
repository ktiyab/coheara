<!-- L4-01: Check-in nudge banner — gentle prompt to record symptoms. -->
<script lang="ts">
  import type { NudgeDecision } from '$lib/types/journal';

  interface Props {
    nudge: NudgeDecision;
    onAccept: () => void;
    onDismiss: () => void;
  }
  let { nudge, onAccept, onDismiss }: Props = $props();
</script>

<div class="mx-6 mb-4 p-4 bg-blue-50 border border-blue-100 rounded-xl">
  <p class="text-sm text-blue-800 mb-3">{nudge.message}</p>
  <div class="flex gap-2">
    <button
      class="px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg text-sm
             font-medium min-h-[44px]"
      onclick={onAccept}
    >
      {nudge.nudge_type === 'PostMedicationChange' ? 'Yes, remind me' : 'Yes'}
    </button>
    <button
      class="px-4 py-2 bg-white text-stone-600 rounded-lg text-sm
             border border-stone-200 min-h-[44px]"
      onclick={onDismiss}
    >
      {nudge.nudge_type === 'PostMedicationChange' ? 'No thanks' : 'Not now'}
    </button>
  </div>
</div>
