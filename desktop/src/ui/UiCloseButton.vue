<script setup lang="ts">
import { INK_FOCUS_CLASSES } from './handDrawn'
const CLOSE_LABEL = 'Fechar'
const CLASSES =
  `inline-flex size-8 shrink-0 cursor-pointer items-center justify-center rounded-md text-muted transition-colors hover:bg-hover hover:text-ink ${INK_FOCUS_CLASSES} motion-reduce:transition-none`

const emit = defineEmits<{ press: [] }>()
</script>

<template>
  <button type="button" :class="CLASSES" :aria-label="CLOSE_LABEL" :title="CLOSE_LABEL" @click="emit('press')">
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      <line x1="18" y1="6" x2="6" y2="18" />
      <line x1="6" y1="6" x2="18" y2="18" />
    </svg>
  </button>
</template>
