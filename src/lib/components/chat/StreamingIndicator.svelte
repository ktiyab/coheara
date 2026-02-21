<script lang="ts">
  import Avatar from '$lib/components/ui/Avatar.svelte';
  import ProcessIndicator from '$lib/components/ui/ProcessIndicator.svelte';
  import { CHAT_STAGES } from '$lib/utils/process-stages';
  import { t } from 'svelte-i18n';

  interface Props {
    documentCount?: number;
  }

  let { documentCount = 0 }: Props = $props();
</script>

<div class="flex items-start gap-2">
  <div class="flex-shrink-0 mt-1">
    <Avatar name={$t('chat.avatar_initial')} variant="ai" size="sm" />
  </div>
  <div class="bg-white dark:bg-gray-900 border border-stone-100 dark:border-gray-800 rounded-2xl rounded-bl-md px-4 py-3 shadow-sm min-w-[240px]">
    <ProcessIndicator stages={CHAT_STAGES} {documentCount} />
  </div>
</div>
