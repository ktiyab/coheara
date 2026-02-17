<!-- R.2+R.3: Reusable error banner with ARIA, severity, action guidance, and dismiss. -->
<script lang="ts">
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

  const styles: Record<Severity, { bg: string; border: string; text: string; icon: string }> = {
    error: {
      bg: 'bg-red-50',
      border: 'border-red-200',
      text: 'text-red-800',
      icon: '!',
    },
    warning: {
      bg: 'bg-amber-50',
      border: 'border-amber-200',
      text: 'text-amber-800',
      icon: '!',
    },
    info: {
      bg: 'bg-blue-50',
      border: 'border-blue-200',
      text: 'text-blue-800',
      icon: 'i',
    },
  };

  let s = $derived(styles[severity]);
</script>

{#if !dismissed}
  <div
    class="rounded-xl border px-4 py-3 {s.bg} {s.border}"
    role="alert"
    aria-live="assertive"
  >
    <div class="flex items-start gap-3">
      <!-- Severity icon -->
      <span class="flex-shrink-0 w-6 h-6 rounded-full flex items-center justify-center
                    text-xs font-bold {severity === 'error' ? 'bg-red-200 text-red-700' :
                    severity === 'warning' ? 'bg-amber-200 text-amber-700' :
                    'bg-blue-200 text-blue-700'}"
            aria-hidden="true">
        {s.icon}
      </span>

      <div class="flex-1 min-w-0">
        <!-- Error message -->
        <p class="text-sm font-medium {s.text}">{message}</p>

        <!-- R.3: Action guidance -->
        {#if guidance}
          <p class="text-xs mt-1 {severity === 'error' ? 'text-red-600' :
              severity === 'warning' ? 'text-amber-600' : 'text-blue-600'}">
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
                 {severity === 'error' ? 'text-red-400 hover:text-red-600' :
                 severity === 'warning' ? 'text-amber-400 hover:text-amber-600' :
                 'text-blue-400 hover:text-blue-600'}"
          onclick={handleDismiss}
          aria-label="Dismiss"
        >
          &times;
        </button>
      {/if}
    </div>
  </div>
{/if}
