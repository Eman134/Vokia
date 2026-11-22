<script lang="ts">
export type HandLineVariant = 'underline' | 'circle' | 'strike' | 'scribble'
</script>

<script setup lang="ts">
import { computed } from 'vue'
import { DEFAULT_JITTER, sketchEllipse, sketchScribble, sketchStrike } from './sketch/sketchPath'

const HAND_PATH = 'M1 3.4C14 2 22 4.6 36 3.2S58 2.2 70 3.6 88 4.4 99 2.8'
const UNDERLINE_VIEWBOX = '0 0 100 6'
const SHAPE_WIDTH = 100
const SHAPE_HEIGHT = 40
const STRIKE_HEIGHT = 8
const SHAPE_SEED = 3
const STROKE_WIDTH = 1.4
// Maior que qualquer traço (em unidades do viewBox ou da tela): o deslocamento inicial esconde o desenho inteiro.
const DRAW_DASH = 4000

const props = withDefaults(defineProps<{ variant?: HandLineVariant; draw?: boolean }>(), {
  variant: 'underline',
  draw: false,
})

const shape = computed(() => {
  switch (props.variant) {
    case 'circle':
      return {
        box: `0 0 ${SHAPE_WIDTH} ${SHAPE_HEIGHT}`,
        d: sketchEllipse(SHAPE_WIDTH, SHAPE_HEIGHT, SHAPE_SEED, DEFAULT_JITTER),
      }
    case 'strike':
      return {
        box: `0 0 ${SHAPE_WIDTH} ${STRIKE_HEIGHT}`,
        d: sketchStrike(SHAPE_WIDTH, STRIKE_HEIGHT, SHAPE_SEED, DEFAULT_JITTER),
      }
    case 'scribble':
      return {
        box: `0 0 ${SHAPE_WIDTH} ${SHAPE_HEIGHT}`,
        d: sketchScribble(SHAPE_WIDTH, SHAPE_HEIGHT, SHAPE_SEED, DEFAULT_JITTER),
      }
    default:
      return { box: UNDERLINE_VIEWBOX, d: HAND_PATH }
  }
})
</script>

<template>
  <svg :viewBox="shape.box" preserveAspectRatio="none" fill="none" stroke="currentColor" aria-hidden="true">
    <path
      :d="shape.d"
      :stroke-width="STROKE_WIDTH"
      stroke-linecap="round"
      vector-effect="non-scaling-stroke"
      :class="draw && 'hand-draw'"
      :style="{ '--hand-dash': DRAW_DASH }"
    />
  </svg>
</template>

<style scoped>
.hand-draw {
  stroke-dasharray: var(--hand-dash);
  animation: hand-draw 180ms ease-out both;
}

@keyframes hand-draw {
  from {
    stroke-dashoffset: var(--hand-dash);
  }
  to {
    stroke-dashoffset: 0;
  }
}

@media (prefers-reduced-motion: reduce) {
  .hand-draw {
    animation: none;
    stroke-dasharray: none;
  }
}
</style>
