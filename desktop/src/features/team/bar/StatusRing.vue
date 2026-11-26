<script setup lang="ts">
import { computed } from 'vue'
import { useFeed } from '../useFeed'
import { useFlow } from '../useFlow'
import { useTeamStatus } from '../useTeamStatus'
import { FLOW_LABEL } from '../flowSteps'
import { PAUSED_DOT_CLASS } from '../agentStateStyle'
import type { Agent } from '../events'
import { useAgentControl } from '../../panel/useAgentControl'
import { MAX_PERCENT } from '../../usage/usagePlans'
import { DEFAULT_JITTER, sketchEllipse } from '../../../ui/sketch/sketchPath'

defineProps<{
  expanded: boolean
  collapsing: boolean
  canAsk: boolean
}>()

const RING_BOX = 28
const RING_CENTER = RING_BOX / 2
const RING_RADIUS = 11
const RING_STROKE = 2.5
const RING_OFFSET = RING_CENTER - RING_RADIUS
const RING_SEED = 5
const RING_PATH = sketchEllipse(RING_RADIUS * 2, RING_RADIUS * 2, RING_SEED, DEFAULT_JITTER)
// O traço irregular não tem raio fixo: pathLength normaliza o comprimento e os dashes ficam em percentual.
const PATH_LENGTH = MAX_PERCENT
const SPINNING_ARC = PATH_LENGTH / 4
const FULL_RING_DASH = `${PATH_LENGTH} 0`
const DRAWING_RING_DASH = `${PATH_LENGTH} ${PATH_LENGTH}`
const SPINNING_RING_DASH = `${SPINNING_ARC} ${PATH_LENGTH - SPINNING_ARC}`

const feed = useFeed()
const flow = useFlow()
const statusStore = useTeamStatus()
const control = useAgentControl()
const needsYou = computed(() => !flow.active && statusStore.status === 'needs-you')
const ringDashArray = computed(() => {
  if (flow.active) return `${flow.progress * PATH_LENGTH} ${PATH_LENGTH}`
  if (needsYou.value) return DRAWING_RING_DASH
  return statusStore.status === 'working' ? SPINNING_RING_DASH : FULL_RING_DASH
})
const stepTitle = computed(() =>
  flow.active ? `Etapa ${flow.step} de ${flow.total} · ${flow.names[flow.step - 1]}` : '',
)
const statusLabel = computed(() => (flow.flow === null ? statusStore.statusText : FLOW_LABEL[flow.flow]))

function dashClass(agent: Agent): string {
  if (control.isPaused(agent)) {
    return PAUSED_DOT_CLASS
  }
  if (feed.permissions.some((permission) => permission.agent === agent)) {
    return 'bg-danger motion-safe:animate-pulse'
  }
  return feed.stateOf(agent) === 'waiting' ? 'bg-warn' : 'bg-ok'
}
</script>

<template>
  <span class="inline-flex shrink-0 items-center" :title="statusLabel">
    <svg
      v-if="feed.activeAgents.length < 2"
      class="size-6.5 shrink-0"
      :class="{ 'animate-spin': !flow.active && statusStore.status === 'working' }"
      :viewBox="`0 0 ${RING_BOX} ${RING_BOX}`"
      fill="none"
      :stroke-width="RING_STROKE"
      aria-hidden="true"
    >
      <g :transform="`translate(${RING_OFFSET} ${RING_OFFSET})`">
        <path :d="RING_PATH" class="stroke-line" />
        <path
          :d="RING_PATH"
          :class="[statusStore.ring, needsYou && 'ring-draw']"
          :pathLength="PATH_LENGTH"
          stroke-linecap="round"
          :stroke-dasharray="ringDashArray"
        />
      </g>
    </svg>
    <span v-else data-testid="agent-dash" class="flex shrink-0 items-center gap-1">
      <span
        v-for="agent in feed.activeAgents"
        :key="agent.id"
        class="h-1 w-3.5 rounded-full"
        :class="dashClass(agent.id)"
      />
    </span>
  </span>
  <span v-if="!expanded || flow.active || !canAsk" class="flex min-w-0 flex-col" :class="expanded ? undefined : 'pr-2'">
    <span
      class="text-label font-semibold"
      :class="[
        statusStore.limitExhausted ? 'text-warn-text' : 'text-muted',
        collapsing ? 'overflow-hidden whitespace-nowrap' : 'truncate',
        { 'max-xl:hidden': !canAsk },
      ]"
      :title="statusLabel"
    >
      {{ statusLabel }}
    </span>
    <span
      v-if="flow.active"
      class="text-meta text-muted max-lg:hidden"
      :class="collapsing ? 'overflow-hidden whitespace-nowrap' : 'truncate'"
      :title="stepTitle"
      >{{ stepTitle }}</span
    >
  </span>
</template>

<style scoped>
.ring-draw {
  animation: ring-draw 1.6s ease-out infinite;
}

@keyframes ring-draw {
  from {
    stroke-dashoffset: 100;
  }
  60%,
  to {
    stroke-dashoffset: 0;
  }
}

@media (prefers-reduced-motion: reduce) {
  .ring-draw {
    animation: none;
  }
}
</style>
