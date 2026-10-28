<script setup lang="ts">
defineProps<{ text: string }>()
</script>

<template>
  <p class="rounded-control bg-accent-soft px-3 py-2 text-caption text-accent">{{ text }}</p>
</template>
