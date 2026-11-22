<script setup lang="ts">
import { computed } from 'vue'
import { DEFAULT_JITTER, sketchEllipse, sketchLine } from './sketch/sketchPath'

const DOT = 12
const GAP = 44
const HEIGHT = 20
const LINE_MARGIN = 3
const DOT_SEED = 3
const LINE_SEED = 40
const STROKE_WIDTH = 1.4

const props = defineProps<{ total: number; current: number }>()

const width = computed(() => props.total * DOT + (props.total - 1) * GAP)
const dots = computed(() =>
  Array.from({ length: props.total }, (_, index) => ({
    index,
    x: index * (DOT + GAP),
    d: sketchEllipse(DOT, DOT, DOT_SEED + index, DEFAULT_JITTER),
  })),
)
const links = computed(() =>
  dots.value.slice(0, -1).map((dot) => ({
    index: dot.index,
    d: sketchLine(dot.x + DOT + LINE_MARGIN, HEIGHT / 2, dot.x + DOT + GAP - LINE_MARGIN, HEIGHT / 2, LINE_SEED + dot.index, DEFAULT_JITTER),
  })),
)
</script>

<template>
  <svg
    :width="width"
    :height="HEIGHT"
    :viewBox="`0 0 ${width} ${HEIGHT}`"
    role="img"
    :aria-label="`Passo ${current + 1} de ${total}`"
    fill="none"
    class="shrink-0"
  >
    <path
      v-for="link in links"
      :key="`link-${link.index}`"
      :d="link.d"
      :stroke-width="STROKE_WIDTH"
      stroke-linecap="round"
      :class="link.index < current ? 'stroke-ink-pen' : 'stroke-graphite'"
    />
    <path
      v-for="dot in dots"
      :key="dot.index"
      :d="dot.d"
      :transform="`translate(${dot.x} ${(HEIGHT - DOT) / 2})`"
      :stroke-width="STROKE_WIDTH"
      stroke-linecap="round"
      :class="dot.index <= current ? 'fill-ink-pen stroke-ink-pen' : 'stroke-graphite'"
    />
  </svg>
</template>
