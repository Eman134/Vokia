<script lang="ts">
export type MarkerTone = 'yellow' | 'green' | 'pink' | 'blue'
</script>

<script setup lang="ts">
import { HAND_DRAWN_RADIUS } from './handDrawn'

const TONE_CLASSES: Record<MarkerTone, string> = {
  yellow: 'bg-mark-yellow',
  green: 'bg-mark-green',
  pink: 'bg-mark-pink',
  blue: 'bg-mark-blue',
}
const BASE_CLASSES = `box-decoration-clone px-1 text-mark-ink ${HAND_DRAWN_RADIUS}`

withDefaults(defineProps<{ tone?: MarkerTone }>(), { tone: 'yellow' })
</script>

<template>
  <mark :class="[BASE_CLASSES, TONE_CLASSES[tone]]"><slot /></mark>
</template>
