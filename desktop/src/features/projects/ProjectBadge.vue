<script setup lang="ts">
import type { ProjectColorKey } from './projectColors'

defineProps<{ color: ProjectColorKey }>()
</script>

<template>
  <svg viewBox="0 0 16 16" aria-hidden="true">
    <circle cx="8" cy="8" r="7.5" :fill="`var(--color-project-${color})`" stroke="var(--color-project-line)" stroke-width="1" />
  </svg>
</template>
