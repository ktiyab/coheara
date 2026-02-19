<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
</script>

<EmptyState
  icon="&#128196;"
  title={$t('home.no_documents')}
  description="{$t('home.empty_description_1')} {$t('home.empty_description_2')}"
  actionLabel={$t('home.empty_cta')}
  onaction={() => navigation.navigate('import')}
/>
