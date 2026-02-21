<!-- LP-08: Empty timeline state — prompts user to load document or tell chat. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import { CalendarIcon } from '$lib/components/icons/md';
</script>

<div class="flex-1 flex flex-col items-center justify-center gap-3">
  <EmptyState
    icon={CalendarIcon}
    title={$t('timeline.empty_title')}
    description={$t('timeline.empty_description')}
    actionLabel={$t('timeline.empty_load_document')}
    onaction={() => navigation.navigate('import')}
  />
  <Button variant="ghost" size="sm" onclick={() => navigation.navigate('chat')}>
    {$t('timeline.empty_tell_chat')}
  </Button>
</div>
