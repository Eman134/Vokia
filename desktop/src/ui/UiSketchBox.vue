<script lang="ts">
import { DEFAULT_JITTER, sketchRect } from './sketch/sketchPath'

export type SketchTone = 'graphite' | 'pen' | 'danger'
export type SketchFill = 'yellow' | 'green' | 'pink' | 'blue'

const DEFAULT_SEED = 1
const SIZE_STEP = 4
const MAX_CACHED_PATHS = 300

// Um único observer para todas as caixas: cada uma só registra o elemento e um callback.
const sizeListeners = new Map<Element, (width: number, height: number) => void>()
let sharedObserver: ResizeObserver | null = null

function watchSize(element: Element, onSize: (width: number, height: number) => void) {
  sharedObserver ??= new ResizeObserver((entries) => {
    for (const entry of entries) {
      sizeListeners.get(entry.target)?.(entry.contentRect.width, entry.contentRect.height)
    }
  })
  sizeListeners.set(element, onSize)
  sharedObserver.observe(element)
}

function unwatchSize(element: Element) {
  sizeListeners.delete(element)
  sharedObserver?.unobserve(element)
  if (sizeListeners.size === 0) {
    sharedObserver?.disconnect()
    sharedObserver = null
  }
}

const pathCache = new Map<string, string>()

function cachedRect(seed: number, width: number, height: number): string {
  const key = `${seed}:${width}:${height}`
  let path = pathCache.get(key)
  if (path === undefined) {
    if (pathCache.size >= MAX_CACHED_PATHS) {
      pathCache.clear()
    }
    path = sketchRect(width, height, seed, DEFAULT_JITTER)
    pathCache.set(key, path)
  }
  return path
}

const snap = (value: number) => (value > 0 ? Math.max(SIZE_STEP, Math.round(value / SIZE_STEP) * SIZE_STEP) : 0)
</script>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

const TONE_COLORS: Record<SketchTone, string> = {
  graphite: 'var(--color-graphite)',
  pen: 'var(--color-ink-pen)',
  danger: 'var(--color-danger)',
}

const props = withDefaults(defineProps<{ seed?: number; tone?: SketchTone; bold?: boolean; fill?: SketchFill }>(), {
  seed: DEFAULT_SEED,
  tone: 'graphite',
  bold: false,
  fill: undefined,
})

const root = ref<HTMLElement | null>(null)
const box = ref({ width: 0, height: 0 })

onMounted(() => {
  if (root.value) {
    watchSize(root.value, (width, height) => {
      const next = { width: snap(width), height: snap(height) }
      if (next.width !== box.value.width || next.height !== box.value.height) {
        box.value = next
      }
    })
  }
})

onBeforeUnmount(() => {
  if (root.value) {
    unwatchSize(root.value)
  }
})

const path = computed(() =>
  box.value.width > 0 && box.value.height > 0 ? cachedRect(props.seed, box.value.width, box.value.height) : '',
)
const strokeStyle = computed(() => ({
  stroke: TONE_COLORS[props.tone],
  strokeWidth: props.bold ? 'var(--stroke-hand-bold)' : 'var(--stroke-hand)',
}))
</script>

<template>
  <div ref="root" class="relative">
    <svg
      v-if="path"
      class="pointer-events-none absolute inset-0 size-full overflow-visible"
      :viewBox="`0 0 ${box.width} ${box.height}`"
      preserveAspectRatio="none"
      fill="none"
      aria-hidden="true"
    >
      <path v-if="fill" :d="path" :style="{ fill: `var(--color-mark-${fill})` }" stroke="none" />
      <path :d="path" :style="strokeStyle" stroke-linecap="round" stroke-linejoin="round" vector-effect="non-scaling-stroke" />
    </svg>
    <div class="relative"><slot /></div>
  </div>
</template>
