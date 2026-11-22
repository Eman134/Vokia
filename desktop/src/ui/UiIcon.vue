<script lang="ts">
const DEFAULT_SIZE = 16
</script>

<script setup lang="ts">
withDefaults(defineProps<{ path: string; size?: number }>(), { size: DEFAULT_SIZE })
</script>

<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 16 16"
    fill="none"
    stroke="currentColor"
    stroke-width="1.4"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path :d="path" />
  </svg>
</template>
