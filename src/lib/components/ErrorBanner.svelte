<!-- R.2+R.3: Reusable error banner with ARIA, severity, action guidance, and dismiss. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { WarningIcon, InfoIcon, CloseIcon } from '$lib/components/icons/md';

  type Severity = 'error' | 'warning' | 'info';

  interface Props {
    message: string;
    severity?: Severity;
    /** R.3: Recovery guidance (e.g. "Try importing the file again.") */
    guidance?: string;
    /** Optional action button label */
    actionLabel?: string;
    /** Callback when action button is clicked */
    onAction?: () => void;
    /** Whether the banner can be dismissed */
    dismissible?: boolean;
    /** Callback when dismissed */
    onDismiss?: () => void;
  }

  let {
    message,
    severity = 'error',
    guidance,
    actionLabel,
    onAction,
    dismissible = true,
    onDismiss,
  }: Props = $props();

  let dismissed = $state(false);

  function handleDismiss() {
    dismissed = true;
    onDismiss?.();
  }

  const styles: Record<Severity, { bg: string; border: string; text: string }> = {
    error: {
      bg: 'bg-[var(--color-danger-50)]',
      border: 'border-[var(--color-danger-200)]',
      text: 'text-[var(--color-danger-800)]',
    },
    warning: {
      bg: 'bg-[var(--color-warning-50)]',
      border: 'border-[var(--color-warning-200)]',
      text: 'text-[var(--color-warning-800)]',
    },
    info: {
      bg: 'bg-[var(--color-info-50)]',
      border: 'border-[var(--color-info-200)]',
      text: 'text-[var(--color-info-800)]',
    },
  };

  let s = $derived(styles[severity]);

  const iconColors: Record<Severity, string> = {
    error: 'text-[var(--color-danger)]',
    warning: 'text-[var(--color-warning)]',
    info: 'text-[var(--color-info)]',
  };
</script>

{#if !dismissed}
  <div
    class="rounded-xl border px-4 py-3 {s.bg} {s.border}"
    role="alert"
    aria-live="assertive"
  >
    <div class="flex items-start gap-3">
      <!-- Severity icon -->
      {#if severity === 'info'}
        <InfoIcon class="w-5 h-5 flex-shrink-0 mt-0.5 {iconColors[severity]}" />
      {:else}
        <WarningIcon class="w-5 h-5 flex-shrink-0 mt-0.5 {iconColors[severity]}" />
      {/if}

      <div class="flex-1 min-w-0">
        <!-- Error message -->
        <p class="text-sm font-medium {s.text}">{message}</p>

        <!-- R.3: Action guidance -->
        {#if guidance}
          <p class="text-xs mt-1 {iconColors[severity]}">
            {guidance}
          </p>
        {/if}

        <!-- Action button -->
        {#if actionLabel && onAction}
          <button
            class="mt-2 text-sm font-medium underline min-h-[44px] {s.text}"
            onclick={onAction}
          >
            {actionLabel}
          </button>
        {/if}
      </div>

      <!-- Dismiss button -->
      {#if dismissible}
        <button
          class="flex-shrink-0 min-h-[44px] min-w-[44px] flex items-center justify-center
                 {severity === 'error' ? 'text-[var(--color-danger-200)] hover:text-[var(--color-danger)]' :
                 severity === 'warning' ? 'text-[var(--color-warning-200)] hover:text-[var(--color-warning)]' :
                 'text-[var(--color-info-200)] hover:text-[var(--color-info)]'}"
          onclick={handleDismiss}
          aria-label={$t('common.dismiss')}
        >
          <CloseIcon class="w-4 h-4" />
        </button>
      {/if}
    </div>
  </div>
{/if}
