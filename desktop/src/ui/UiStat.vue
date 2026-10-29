<script setup lang="ts">
import { computed } from 'vue'
import { OUTER_RING_RADIUS, TONE_STROKE, ringDash } from '../features/usage/usagePlans'
import UiProgress, { toPercent, type ProgressTone } from './UiProgress.vue'

const RING_BOX = 44
const RING_CENTER = RING_BOX / 2
const RING_STROKE = 2.4
const RING_STROKE_CLASS: Record<ProgressTone, string> = { accent: 'stroke-accent', ok: 'stroke-ok', ...TONE_STROKE }
const TREND_TEXT: Record<'up' | 'down', string> = { up: 'text-ok', down: 'text-danger' }
const TREND_ARROW: Record<'up' | 'down', string> = { up: '↑', down: '↓' }

const props = withDefaults(
  defineProps<{
    label: string
    value: string
    variant: 'progress' | 'ring' | 'trend'
    percent?: number
    tone?: ProgressTone
    note?: string
    trend?: { direction: 'up' | 'down'; text: string }
  }>(),
  { tone: 'accent' },
)

const percent = computed(() => toPercent(props.percent ?? 0))
</script>

<template>
  <article class="flex flex-col gap-2 rounded-card border border-line bg-surface p-3.5">
    <p class="text-caption text-muted">{{ label }}</p>
    <div class="flex items-center justify-between gap-3">
      <div class="flex min-w-0 flex-col gap-1">
        <p class="text-section font-semibold tabular-nums text-ink">{{ value }}</p>
        <p v-if="variant === 'trend' && trend" class="text-caption" :class="TREND_TEXT[trend.direction]">
          <span aria-hidden="true">{{ TREND_ARROW[trend.direction] }}</span> {{ trend.text }}
        </p>
        <p v-if="note" class="text-small text-muted">{{ note }}</p>
      </div>
      <div v-if="variant === 'ring'" class="relative grid size-11 flex-none place-items-center">
        <svg class="absolute inset-0 size-11" :viewBox="`0 0 ${RING_BOX} ${RING_BOX}`" aria-hidden="true">
          <circle
            :cx="RING_CENTER"
            :cy="RING_CENTER"
            :r="OUTER_RING_RADIUS"
            fill="none"
            :stroke-width="RING_STROKE"
            class="stroke-line"
          />
          <circle
            v-if="percent > 0"
            :cx="RING_CENTER"
            :cy="RING_CENTER"
            :r="OUTER_RING_RADIUS"
            fill="none"
            :stroke-width="RING_STROKE"
            stroke-linecap="round"
            :class="RING_STROKE_CLASS[tone]"
            :stroke-dasharray="ringDash(percent, OUTER_RING_RADIUS)"
            :transform="`rotate(-90 ${RING_CENTER} ${RING_CENTER})`"
          />
        </svg>
        <span class="text-tiny font-medium tabular-nums text-ink">{{ percent }}%</span>
      </div>
    </div>
    <UiProgress v-if="variant === 'progress'" :value="percent" :tone="tone" />
  </article>
</template>
