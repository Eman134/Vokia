<script setup lang="ts">
defineProps<{ command: string }>()
</script>

<template>
  <pre
    class="min-w-0 overflow-x-auto rounded-card border border-line bg-surface-2 px-3 py-2.5 font-mono text-small text-ink"
  >{{ command }}</pre>
</template>
