<script setup lang="ts">
import { ref, useId, watch } from 'vue'
import { storeToRefs } from 'pinia'
import { useUsage } from './useUsage'
import UsagePanel from './UsagePanel.vue'
import {
  OUTER_RING_RADIUS,
  PLANS,
  TONE_BG,
  TONE_STROKE,
  TONE_TEXT,
  WARN_THRESHOLD,
  clampPercent,
  ringDash,
  toneOf,
  type PlanId,
} from './usagePlans'

defineProps<{ expanded: boolean }>()

const RING_BOX = 44
const RING_CENTER = RING_BOX / 2
const RING_STROKE = 2.4
const PANEL_GAP = 10
const PANEL_LEFT = RING_BOX + PANEL_GAP
const RINGS: readonly { plan: PlanId; radius: number }[] = [{ plan: 'cc', radius: OUTER_RING_RADIUS }]
const USER_INITIAL = 'K'
const USER_NAME = 'Kay'

const store = useUsage()
const { claude, visible, alert, ariaLabel, openable } = storeToRefs(store)
const open = ref(false)
const panelId = useId()

watch(openable, (isOpenable) => {
  if (!isOpenable) open.value = false
})

function toggle() {
  open.value = !open.value
  if (open.value) store.refreshClock()
}

function emphasisClass(percent: number) {
  return percent >= WARN_THRESHOLD ? ['font-semibold', TONE_TEXT[toneOf(percent)]] : []
}
</script>

<template>
  <div class="relative flex flex-col" @keydown.esc="open = false">
    <button
      type="button"
      :class="[
        'mt-1 flex items-center gap-2.5 rounded-control-lg border-0 bg-transparent text-left',
        openable ? 'cursor-pointer' : 'cursor-default',
        expanded ? 'p-0 pr-1' : 'p-0',
      ]"
      :style="{ height: `${RING_BOX}px` }"
      :disabled="!openable"
      :aria-label="ariaLabel"
      :title="ariaLabel"
      :aria-expanded="open"
      :aria-controls="open ? panelId : undefined"
      @click="toggle"
    >
      <span class="relative grid size-11 flex-none place-items-center">
        <svg
          v-if="claude && visible"
          class="absolute inset-0 size-11"
          :viewBox="`0 0 ${RING_BOX} ${RING_BOX}`"
          aria-hidden="true"
        >
          <template v-for="ring in RINGS" :key="ring.plan">
            <circle
              :cx="RING_CENTER"
              :cy="RING_CENTER"
              :r="ring.radius"
              fill="none"
              :stroke-width="RING_STROKE"
              class="stroke-line"
            />
            <circle
              :cx="RING_CENTER"
              :cy="RING_CENTER"
              :r="ring.radius"
              fill="none"
              :stroke-width="RING_STROKE"
              stroke-linecap="round"
              :class="TONE_STROKE[toneOf(claude.fiveHour.percent)]"
              :stroke-dasharray="ringDash(claude.fiveHour.percent, ring.radius)"
              :transform="`rotate(-90 ${RING_CENTER} ${RING_CENTER})`"
            />
          </template>
        </svg>
        <span
          title="Você"
          class="relative grid size-7 place-items-center rounded-full border border-line bg-surface-2 text-small font-semibold text-ink"
          >{{ USER_INITIAL }}</span
        >
        <span
          v-if="claude && alert"
          class="absolute top-px right-px size-2.25 rounded-full shadow-[0_0_0_2px_var(--color-surface)]"
          :class="TONE_BG[toneOf(claude.fiveHour.percent)]"
        />
      </span>
      <span v-if="expanded" class="flex min-w-0 flex-1 flex-col gap-1">
        <b class="text-caption font-medium text-ink">{{ USER_NAME }}</b>
        <template v-if="claude && visible">
          <span v-for="plan in PLANS" :key="plan.id" class="flex items-center gap-1.5 text-tiny text-muted">
            <span class="w-10 flex-none">{{ plan.short }} 5h</span>
            <span class="h-1 flex-1 overflow-hidden rounded-full bg-line">
              <span
                class="block h-full rounded-full"
                :class="TONE_BG[toneOf(claude.fiveHour.percent)]"
                :style="{ width: `${clampPercent(claude.fiveHour.percent)}%` }"
              />
            </span>
            <span class="w-7 flex-none text-right tabular-nums" :class="emphasisClass(claude.fiveHour.percent)">{{
              claude.fiveHour.percent
            }}%</span>
          </span>
        </template>
      </span>
    </button>
    <UsagePanel v-if="open" :panel-id="panelId" :style="{ left: `${PANEL_LEFT}px` }" @close="toggle" />
  </div>
</template>
