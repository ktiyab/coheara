<!-- MD: SendIcon -->
<script lang="ts">
  import MdIcon from '../MdIcon.svelte';
  let { class: className = 'w-5 h-5' }: { class?: string } = $props();
</script>
<MdIcon d="M160-220v-520l616.92 260L160-220Zm30.77-47.62L697.08-480 190.77-693.85v168L385.85-480l-195.08 44.62v167.76Zm0 0v-426.23 426.23Z" class={className} />
