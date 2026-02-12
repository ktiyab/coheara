<script lang="ts">
  // No props — displays "Searching your documents..." with animated dots
</script>

<div class="flex items-start gap-2">
  <div class="w-8 h-8 rounded-full bg-teal-600 flex items-center justify-center
              text-white text-sm font-bold flex-shrink-0 mt-1">
    C
  </div>
  <div class="bg-white border border-stone-100 rounded-2xl rounded-bl-md px-4 py-3 shadow-sm">
    <div class="flex items-center gap-1.5">
      <span class="w-2 h-2 rounded-full bg-stone-300 animate-bounce" style="animation-delay: 0ms"></span>
      <span class="w-2 h-2 rounded-full bg-stone-300 animate-bounce" style="animation-delay: 150ms"></span>
      <span class="w-2 h-2 rounded-full bg-stone-300 animate-bounce" style="animation-delay: 300ms"></span>
    </div>
    <p class="text-xs text-stone-400 mt-1">Searching your documents...</p>
  </div>
</div>
