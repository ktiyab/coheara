<!-- L4-01: Check-in nudge banner — gentle prompt to record symptoms. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { NudgeDecision } from '$lib/types/journal';
  import Button from '$lib/components/ui/Button.svelte';

  interface Props {
    nudge: NudgeDecision;
    onAccept: () => void;
    onDismiss: () => void;
  }
  let { nudge, onAccept, onDismiss }: Props = $props();
</script>

<div class="mx-6 mb-4 p-4 bg-[var(--color-info-50)] border border-[var(--color-info-200)] rounded-xl">
  <p class="text-sm text-[var(--color-info-800)] mb-3">{nudge.message}</p>
  <div class="flex gap-2">
    <Button variant="primary" size="sm" onclick={onAccept}>
      {nudge.nudge_type === 'PostMedicationChange' ? $t('journal.nudge_accept_post_med') : $t('journal.nudge_accept_default')}
    </Button>
    <Button variant="secondary" size="sm" onclick={onDismiss}>
      {nudge.nudge_type === 'PostMedicationChange' ? $t('journal.nudge_dismiss_post_med') : $t('journal.nudge_dismiss_default')}
    </Button>
  </div>
</div>
