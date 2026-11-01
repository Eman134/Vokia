<script setup lang="ts">
import { computed } from 'vue'
import { projectIconOf } from './projectIcons'

const props = defineProps<{ icon: string }>()

const path = computed(() => projectIconOf(props.icon).path)
</script>

<template>
  <svg
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
