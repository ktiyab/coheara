<!-- MD: CloseIcon -->
<script lang="ts">
  import MdIcon from '../MdIcon.svelte';
  let { class: className = 'w-5 h-5' }: { class?: string } = $props();
</script>
<MdIcon d="M256-213.85 213.85-256l224-224-224-224L256-746.15l224 224 224-224L746.15-704l-224 224 224 224L704-213.85l-224-224-224 224Z" class={className} />
