<!--
  CT-01: ModelTagEditor — Inline per-model tag editor.

  Shows all capability tags as toggleable chips.
  Active tags are highlighted, inactive are dimmed.
  Calls IPC to persist tag changes immediately.
-->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { CapabilityTag } from '$lib/types/ai';
  import { ALL_CAPABILITY_TAGS } from '$lib/types/ai';
  import { addModelTag, removeModelTag } from '$lib/api/ai';
  import { ai } from '$lib/stores/ai.svelte';
  import ModelTagChip from './ModelTagChip.svelte';

  interface Props {
    modelName: string;
  }

  let { modelName }: Props = $props();

  const activeTags = $derived(ai.getTagsForModel(modelName));

  function isActive(tag: CapabilityTag): boolean {
    return activeTags.includes(tag);
  }

  async function handleToggle(tag: CapabilityTag) {
    const wasActive = isActive(tag);
    try {
      if (wasActive) {
        await removeModelTag(modelName, tag);
        ai.setTagsForModel(modelName, activeTags.filter((t) => t !== tag));
      } else {
        await addModelTag(modelName, tag);
        ai.setTagsForModel(modelName, [...activeTags, tag]);
      }
    } catch {
      // Silent — IPC failure, local state not changed
    }
  }
</script>

<div
  class="flex flex-wrap gap-1.5 mt-2"
  role="group"
  aria-label={$t('ai.capability_tags_aria')}
>
  {#each ALL_CAPABILITY_TAGS as tag (tag)}
    <ModelTagChip
      {tag}
      active={isActive(tag)}
      toggleable
      ontoggle={handleToggle}
    />
  {/each}
</div>
