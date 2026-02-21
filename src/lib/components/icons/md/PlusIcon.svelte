<!-- MD: PlusIcon -->
<script lang="ts">
  import MdIcon from '../MdIcon.svelte';
  let { class: className = 'w-5 h-5' }: { class?: string } = $props();
</script>
<MdIcon d="M464.62-464.62H240v-30.76h224.62V-720h30.76v224.62H720v30.76H495.38V-240h-30.76v-224.62Z" class={className} />
