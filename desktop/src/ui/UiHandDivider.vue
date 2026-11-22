<script setup lang="ts">
import { DEFAULT_JITTER, sketchLine } from './sketch/sketchPath'

const WIDTH = 6
const HEIGHT = 1000
const DEFAULT_SEED = 7
const STROKE_WIDTH = 1.4

// O viewBox é largo o bastante para o desvio do traço caber; quem usa posiciona e dá a altura.
const props = withDefaults(defineProps<{ seed?: number }>(), { seed: DEFAULT_SEED })
const path = sketchLine(WIDTH / 2, 0, WIDTH / 2, HEIGHT, props.seed, DEFAULT_JITTER)
</script>

<template>
  <svg
    class="pointer-events-none w-1.5 text-graphite"
    :viewBox="`0 0 ${WIDTH} ${HEIGHT}`"
    preserveAspectRatio="none"
    fill="none"
    aria-hidden="true"
  >
    <path :d="path" :stroke-width="STROKE_WIDTH" stroke="currentColor" stroke-linecap="round" vector-effect="non-scaling-stroke" />
  </svg>
</template>
