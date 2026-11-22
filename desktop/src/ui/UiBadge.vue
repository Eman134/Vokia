<script lang="ts">
export const BADGE_STATUS = ['trabalhando', 'precisa de você', 'livre', 'erro', 'backlog'] as const
</script>

<script setup lang="ts">
import UiHandLine from './UiHandLine.vue'
import UiIcon from './UiIcon.vue'
import { DEFAULT_JITTER, sketchCheck } from './sketch/sketchPath'
import { useHighlightOnce } from './useHighlightOnce'

type BadgeStatus = (typeof BADGE_STATUS)[number] | 'lote'

const props = defineProps<{
  status: BadgeStatus
  label?: string
  highlight?: boolean
}>()

const HIGHLIGHT_KEY = 'ui-badge'
const GLYPH_SIZE = 12
const CHECK_BOX = 16
const CHECK_SEED = 11
const CHECK_PATH = sketchCheck(CHECK_BOX, CHECK_BOX, CHECK_SEED, DEFAULT_JITTER)
const BASE_CLASSES = 'relative inline-flex h-6 shrink-0 items-center rounded-sm px-2 text-caption font-medium'
const GLYPH_CLASSES = 'mr-1 flex-none'
const CIRCLE_CLASSES =
  'pointer-events-none absolute -inset-x-1 -inset-y-0.5 h-[calc(100%+4px)] w-[calc(100%+8px)]'
const GRAPHITE_LINE_CLASSES = 'pointer-events-none absolute inset-x-1 bottom-0.5 h-1.5 text-graphite'
// Cada estado tem marca-texto (cor) e forma (tique, círculo, rabisco, traço), para não depender só da cor.
const TEXT_CLASSES: Record<BadgeStatus, string> = {
  trabalhando: 'text-mark-ink',
  'precisa de você': 'text-mark-ink',
  livre: 'text-faint',
  erro: 'text-mark-ink',
  backlog: 'text-muted',
  lote: 'text-accent',
}
const SURFACE_CLASSES: Record<BadgeStatus, string> = {
  trabalhando: 'bg-mark-green',
  'precisa de você': 'bg-mark-yellow',
  livre: '',
  erro: 'bg-mark-pink',
  backlog: '',
  lote: 'bg-accent-soft',
}
const HIGHLIGHT_SURFACE_CLASSES = 'bg-sticky'

// Lido uma vez no setup: o destaque acontece na montagem do primeiro selo marcado.
const highlighted = props.highlight && useHighlightOnce(HIGHLIGHT_KEY)
</script>

<template>
  <span
    :class="[
      BASE_CLASSES,
      TEXT_CLASSES[status],
      highlighted ? HIGHLIGHT_SURFACE_CLASSES : SURFACE_CLASSES[status],
    ]"
  >
    <UiIcon v-if="status === 'trabalhando'" :path="CHECK_PATH" :size="GLYPH_SIZE" :class="GLYPH_CLASSES" />
    <UiHandLine v-else-if="status === 'erro'" variant="scribble" :class="[GLYPH_CLASSES, 'h-2.5 w-3']" />
    {{ label ?? status }}
    <UiHandLine v-if="status === 'precisa de você'" variant="circle" :class="CIRCLE_CLASSES" />
    <UiHandLine v-else-if="status === 'backlog'" :class="GRAPHITE_LINE_CLASSES" />
  </span>
</template>
