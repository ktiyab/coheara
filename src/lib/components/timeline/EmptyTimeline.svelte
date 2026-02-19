<!-- L4-04: Empty timeline state — prompts user to load first document. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
</script>

<div class="flex-1 flex items-center justify-center">
  <EmptyState
    icon="&#x1F4C5;"
    title={$t('timeline.empty_title')}
    description={$t('timeline.empty_description')}
    actionLabel={$t('timeline.empty_load_document')}
    onaction={() => navigation.navigate('import')}
  />
</div>
