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
}>()

const BAR_BG: Record<ProgressTone, string> = { accent: 'bg-accent', ok: 'bg-ok', ...TONE_BG }

const percent = computed(() => toPercent(props.value))
const barTone = computed<ProgressTone>(() => props.tone ?? (props.faixa ? toneOf(percent.value) : 'accent'))
</script>

<template>
  <div
    role="progressbar"
    aria-valuemin="0"
    :aria-valuemax="MAX_PERCENT"
    :aria-valuenow="percent"
    class="h-1.5 w-full overflow-hidden rounded-full bg-surface-2"
  >
    <span class="block h-full rounded-full" :class="BAR_BG[barTone]" :style="{ width: `${percent}%` }" />
  </div>
</template>
