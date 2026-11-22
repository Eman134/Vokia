<script setup lang="ts">
defineProps<{ text: string }>()
</script>

<template>
  <p class="border-l-2 border-graphite py-1 pl-3 font-hand text-annotation text-muted">{{ text }}</p>
</template>
