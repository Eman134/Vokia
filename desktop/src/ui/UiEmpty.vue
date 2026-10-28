<script setup lang="ts">
defineProps<{
  title: string
  text: string
}>()
</script>

<template>
  <div class="flex flex-col items-center gap-2 text-center">
    <slot name="mascot" />
    <p class="text-section text-ink">{{ title }}</p>
    <p class="text-body text-muted">{{ text }}</p>
  </div>
</template>
