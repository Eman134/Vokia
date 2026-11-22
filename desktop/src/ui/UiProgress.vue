<script lang="ts">
import { clampPercent } from '../features/usage/usagePlans'

export type ProgressTone = 'accent' | 'ok' | 'warn' | 'danger' | 'ink'

export function toPercent(value: number): number {
  const numeric = Number(value)
  return clampPercent(Math.max(0, Number.isNaN(numeric) ? 0 : numeric))
}
</script>

<script setup lang="ts">
import { computed } from 'vue'
import { MAX_PERCENT, TONE_BG, toneOf } from '../features/usage/usagePlans'

const props = defineProps<{
  value: number
  tone?: ProgressTone
  faixa?: boolean
  pencil?: boolean
}>()

const BAR_BG: Record<ProgressTone, string> = { accent: 'bg-accent', ok: 'bg-ok', ...TONE_BG }
const PENCIL_TEXT: Record<ProgressTone, string> = {
  accent: 'text-accent',
  ok: 'text-ok',
  warn: 'text-warn',
  danger: 'text-danger',
  ink: 'text-ink',
}
const TRACK_CLASSES = 'h-1.5 w-full overflow-hidden rounded-full bg-surface-2'
const PENCIL_TRACK_CLASSES = 'h-2.5 w-full overflow-hidden rounded-sm border border-graphite bg-surface'
// Barra de lápis: o preenchimento é hachura diagonal, com um traço fechando a ponta.
const PENCIL_FILL_CLASSES =
  'block h-full border-r border-current bg-[repeating-linear-gradient(135deg,currentColor_0_1.5px,transparent_1.5px_5px)]'

const percent = computed(() => toPercent(props.value))
const barTone = computed<ProgressTone>(() => props.tone ?? (props.faixa ? toneOf(percent.value) : 'accent'))
</script>

<template>
  <div
    role="progressbar"
    aria-valuemin="0"
    :aria-valuemax="MAX_PERCENT"
    :aria-valuenow="percent"
    :class="pencil ? PENCIL_TRACK_CLASSES : TRACK_CLASSES"
  >
    <span
      :class="pencil ? [PENCIL_FILL_CLASSES, PENCIL_TEXT[barTone]] : ['block h-full rounded-full', BAR_BG[barTone]]"
      :style="{ width: `${percent}%` }"
    />
  </div>
</template>
