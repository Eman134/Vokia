<script lang="ts">
export type AnnotationDirection = 'left' | 'right' | 'up' | 'down'
</script>

<script setup lang="ts">
import { computed } from 'vue'
import { DEFAULT_JITTER, sketchArrow } from './sketch/sketchPath'

const SHORT = 22
const LONG = 38
const MARGIN = 3
const ARROW_SEED = 5
const STROKE_WIDTH = 1.4

const props = withDefaults(defineProps<{ text: string; direction?: AnnotationDirection }>(), {
  direction: 'right',
})

const horizontal = computed(() => props.direction === 'left' || props.direction === 'right')
// A seta aponta para o alvo: vem antes do texto quando o alvo está à esquerda ou acima.
const arrowFirst = computed(() => props.direction === 'left' || props.direction === 'up')
const arrow = computed(() => {
  const width = horizontal.value ? LONG : SHORT
  const height = horizontal.value ? SHORT : LONG
  const middle = (horizontal.value ? height : width) / 2
  const far = (horizontal.value ? width : height) - MARGIN
  const [from, to] = arrowFirst.value ? [far, MARGIN] : [MARGIN, far]
  const d = horizontal.value
    ? sketchArrow(from, middle, to, middle, ARROW_SEED, DEFAULT_JITTER)
    : sketchArrow(middle, from, middle, to, ARROW_SEED, DEFAULT_JITTER)
  return { width, height, d }
})
</script>

<template>
  <span :class="['inline-flex items-center gap-1 text-muted', horizontal ? 'flex-row' : 'flex-col']">
    <svg
      :class="['flex-none', arrowFirst ? 'order-first' : 'order-last']"
      :width="arrow.width"
      :height="arrow.height"
      :viewBox="`0 0 ${arrow.width} ${arrow.height}`"
      fill="none"
      stroke="currentColor"
      aria-hidden="true"
    >
      <path :d="arrow.d" :stroke-width="STROKE_WIDTH" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
    <span class="font-hand text-annotation">{{ text }}</span>
  </span>
</template>
