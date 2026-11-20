<script setup lang="ts">
import { computed } from 'vue'
import { useFeed } from '../useFeed'
import { useFlow } from '../useFlow'
import { useTeamStatus } from '../useTeamStatus'
import { FLOW_LABEL } from '../flowSteps'
import { PAUSED_DOT_CLASS } from '../agentStateStyle'
import type { Agent } from '../events'
import { useAgentControl } from '../../panel/useAgentControl'
import { MAX_PERCENT, ringDash } from '../../usage/usagePlans'

defineProps<{
  expanded: boolean
  collapsing: boolean
  canAsk: boolean
}>()

const RING_BOX = 28
const RING_CENTER = RING_BOX / 2
const RING_RADIUS = 11
const RING_STROKE = 2.5
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS
const SPINNING_ARC = RING_CIRCUMFERENCE / 4
const FULL_RING_DASH = `${RING_CIRCUMFERENCE} 0`
const SPINNING_RING_DASH = `${SPINNING_ARC} ${RING_CIRCUMFERENCE - SPINNING_ARC}`

const feed = useFeed()
const flow = useFlow()
const statusStore = useTeamStatus()
const control = useAgentControl()
const ringDashArray = computed(() =>
  flow.active ? ringDash(flow.progress * MAX_PERCENT, RING_RADIUS) : statusStore.status === 'working' ? SPINNING_RING_DASH : FULL_RING_DASH,
)
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
  <svg
    v-if="feed.activeAgents.length < 2"
    class="size-6.5 shrink-0"
    :class="{
      'animate-spin': !flow.active && statusStore.status === 'working',
      'animate-pulse motion-reduce:animate-none': !flow.active && statusStore.status === 'needs-you',
    }"
    :viewBox="`0 0 ${RING_BOX} ${RING_BOX}`"
    fill="none"
    :stroke-width="RING_STROKE"
    aria-hidden="true"
  >
    <circle :cx="RING_CENTER" :cy="RING_CENTER" :r="RING_RADIUS" class="stroke-line" />
    <circle
      :cx="RING_CENTER"
      :cy="RING_CENTER"
      :r="RING_RADIUS"
      :class="statusStore.ring"
      stroke-linecap="round"
      :stroke-dasharray="ringDashArray"
    />
  </svg>
  <span v-else data-testid="agent-dash" class="flex shrink-0 items-center gap-1">
    <span
      v-for="agent in feed.activeAgents"
      :key="agent.id"
      class="h-1 w-3.5 rounded-full"
      :class="dashClass(agent.id)"
    />
  </span>
  <span class="flex min-w-0 flex-col" :class="expanded ? undefined : 'pr-2'">
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
