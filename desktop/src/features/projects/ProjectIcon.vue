<script setup lang="ts">
import { computed } from 'vue'
import { projectIconOf } from './projectIcons'
import type { ProjectColorKey } from './projectColors'

const props = defineProps<{ icon: string; color?: ProjectColorKey }>()

const path = computed(() => projectIconOf(props.icon).path)
const colorStyle = computed(() =>
  props.color ? { color: `var(--color-project-${props.color})` } : undefined,
)
</script>

<template>
  <svg
    viewBox="0 0 16 16"
    fill="currentColor"
    stroke="currentColor"
    stroke-width="1.4"
    stroke-linecap="round"
    stroke-linejoin="round"
    :style="colorStyle"
    aria-hidden="true"
  >
    <path :d="path" />
  </svg>
</template>
