<!-- MD: HomeIcon -->
<script lang="ts">
  import MdIcon from '../MdIcon.svelte';
  let { class: className = 'w-5 h-5' }: { class?: string } = $props();
</script>
<MdIcon d="M230.77-190.77h161.54v-240h175.38v240h161.54v-373.85L480-753.46 230.77-564.87v374.1ZM200-160v-420l280-211.54L760-580v420H536.92v-240H423.08v240H200Zm280-312.23Z" class={className} />
