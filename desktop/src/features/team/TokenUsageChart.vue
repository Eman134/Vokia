<script setup lang="ts">
import { computed, useId } from 'vue'
import type { TurnUsage } from './events'
import { formatUsageDetails, tokenSplit } from './messageUsageFormat'

const props = defineProps<{ usage?: TurnUsage }>()

const DONUT_VIEWBOX = 16
const DONUT_CENTER = DONUT_VIEWBOX / 2
const DONUT_RADIUS = 5.5
const DONUT_STROKE = 4
const FULL_TURN_DEGREES = 360
const CIRCUMFERENCE = 2 * Math.PI * DONUT_RADIUS
const DIMMED_OPACITY = 0.4
const TRIGGER_CLASSES =
  'flex cursor-default items-center gap-1 rounded-control focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-accent'
const TOOLTIP_CLASSES =
  'pointer-events-none invisible absolute right-0 top-full z-30 mt-1.5 flex w-56 flex-col gap-1 rounded-lg border border-line bg-surface p-3 text-xs opacity-0 shadow-float transition-opacity duration-150 group-hover/donut:visible group-hover/donut:opacity-100 group-focus-within/donut:visible group-focus-within/donut:opacity-100 motion-reduce:transition-none'

const tooltipId = useId()
const split = computed(() => tokenSplit(props.usage))
const details = computed(() => formatUsageDetails(props.usage))

const arcs = computed(() => {
  if (split.value === null) return []
  const { inputDegrees, outputDegrees, dominant } = split.value
  const inputLength = (inputDegrees / FULL_TURN_DEGREES) * CIRCUMFERENCE
  const outputLength = (outputDegrees / FULL_TURN_DEGREES) * CIRCUMFERENCE
  return [
    { side: 'in', stroke: 'stroke-accent', length: inputLength, offset: 0, dimmed: dominant !== 'in' },
    { side: 'out', stroke: 'stroke-ok', length: outputLength, offset: -inputLength, dimmed: dominant !== 'out' },
  ].filter((arc) => arc.length > 0)
})
</script>

<template>
  <span v-if="split" class="group/donut relative inline-flex" data-testid="token-donut">
    <span tabindex="0" :class="TRIGGER_CLASSES" :aria-describedby="tooltipId" :aria-label="split.label">
      <svg
        class="size-4 shrink-0 -rotate-90"
        :viewBox="`0 0 ${DONUT_VIEWBOX} ${DONUT_VIEWBOX}`"
        fill="none"
        aria-hidden="true"
      >
        <circle
          v-for="arc in arcs"
          :key="arc.side"
          :class="arc.stroke"
          :cx="DONUT_CENTER"
          :cy="DONUT_CENTER"
          :r="DONUT_RADIUS"
          :stroke-width="DONUT_STROKE"
          :stroke-dasharray="`${arc.length} ${CIRCUMFERENCE}`"
          :stroke-dashoffset="arc.offset"
          :opacity="arc.dimmed ? DIMMED_OPACITY : 1"
        />
      </svg>
      <span class="tabular-nums text-xs text-muted">{{ split.label }}</span>
    </span>
    <span :id="tooltipId" role="tooltip" :class="TOOLTIP_CLASSES">
      <span v-for="detail in details" :key="detail.label" class="flex items-baseline justify-between gap-3">
        <span class="text-muted">{{ detail.label }}</span>
        <span class="truncate text-right font-medium tabular-nums text-ink">{{ detail.value }}</span>
      </span>
    </span>
  </span>
</template>
