<script lang="ts">
  import type { CriticalLabAlert } from '$lib/types/trust';

  interface Props {
    alerts: CriticalLabAlert[];
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { alerts, onNavigate }: Props = $props();
</script>

<div class="px-6 py-2">
  {#each alerts as alert (alert.id)}
    <div class="bg-red-50 border border-red-200 rounded-xl p-4 mb-2"
         role="alert"
         aria-label="Critical lab result: {alert.test_name}">
      <div class="flex items-start gap-3">
        <span class="text-red-600 mt-0.5 flex-shrink-0 text-lg" aria-hidden="true">&#9888;</span>
        <div class="flex-1">
          <p class="text-sm font-semibold text-red-800">
            Critical: {alert.test_name}
          </p>
          <p class="text-sm text-red-700 mt-1">
            {alert.value} {alert.unit}
            {#if alert.reference_range}
              <span class="text-red-500">(ref: {alert.reference_range})</span>
            {/if}
          </p>
          <button
            class="text-sm text-red-700 font-medium mt-2 underline
                   min-h-[44px] min-w-[44px] -ml-1 px-1"
            onclick={() => onNavigate('document-detail', { documentId: alert.document_id })}
          >
            View source document
          </button>
        </div>
      </div>
    </div>
  {/each}
</div>
