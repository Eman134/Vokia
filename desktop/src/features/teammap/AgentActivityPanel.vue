<script setup lang="ts">
import { watch } from 'vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiHandDivider from '../../ui/UiHandDivider.vue'
import UiHandLine from '../../ui/UiHandLine.vue'
import { presenceOf } from '../team/agentStateStyle'
import type { Agent, AgentState } from '../team/events'
import { projectAgentKey } from '../team/projectAgentKey'
import { useFeed } from '../team/useFeed'
import { useRoster } from '../team/useRoster'
import { useAgentControl } from '../panel/useAgentControl'
import { ACTIVITY_COPY } from './activityCopy'
import type { AgentRun } from './agentActivity'
import { useAgentActivity } from './useAgentActivity'

const PORTRAIT_SIZE = 28
const MARKER_CLASSES = 'absolute -left-8 top-1 grid size-5 place-items-center text-ink'
const MARKER_CIRCLE_CLASSES = 'pointer-events-none absolute inset-0 size-full'
// Cada estado tem selo (marca-texto + forma) no UiBadge; pausado cai no neutro, como na tela Agora.
const BADGE_BY_STATE: Record<AgentState, 'trabalhando' | 'precisa de você' | 'livre'> = {
  waiting: 'precisa de você',
  thinking: 'trabalhando',
  sending: 'trabalhando',
  idle: 'livre',
}
// Do que mais pede atenção ao que menos: o card mostra o estado de maior prioridade entre as execuções.
const STATE_PRIORITY: readonly AgentState[] = ['waiting', 'thinking', 'sending', 'idle']

defineProps<{ agents: readonly Agent[] }>()

const feed = useFeed()
const roster = useRoster()
const control = useAgentControl()
const activity = useAgentActivity()

// Estado de cada execução vem do feed; muda a qualquer momento, então recarrega junto com ele.
watch([() => feed.states, () => feed.activity], () => void activity.load(), { immediate: true, deep: true })

function stateOf(runs: readonly AgentRun[]): AgentState {
  const states = runs.map((run) => feed.activity[projectAgentKey(run.projectId, run.instanceId)] ?? 'idle')
  return STATE_PRIORITY.find((state) => states.includes(state)) ?? 'idle'
}

// Pausado só vale sem execução em andamento (o controle de pausa de hoje é por projeto).
function presence(agent: Agent, runs: readonly AgentRun[]) {
  return presenceOf(stateOf(runs), runs.length === 0 && control.isPaused(agent))
}

function badgeOf(agent: Agent, runs: readonly AgentRun[]) {
  return runs.length === 0 && control.isPaused(agent) ? 'backlog' : BADGE_BY_STATE[stateOf(runs)]
}

function projectNames(runs: readonly AgentRun[]): string[] {
  return [...new Set(runs.map((run) => run.projectName))]
}

function runSummary(run: AgentRun): string {
  return run.itemId === null ? run.projectName : `${run.projectName} · ${run.itemId}${run.itemTitle ? ` ${run.itemTitle}` : ''}`
}
</script>

<template>
  <section v-if="agents.length > 0" class="flex flex-col gap-2">
    <h3 class="text-label font-medium text-muted">{{ ACTIVITY_COPY.title }}</h3>
    <ul class="relative m-0 flex list-none flex-col gap-4 py-1 pl-8">
      <UiHandDivider class="absolute inset-y-0 left-2 h-full" />
      <li v-for="agent in agents" :key="agent" class="relative flex min-w-0 flex-col gap-1.5">
        <span :class="MARKER_CLASSES" aria-hidden="true">
          <UiHandLine variant="circle" :class="MARKER_CIRCLE_CLASSES" />
          <span class="size-1.5 rounded-full" :class="presence(agent, activity.runsOf(agent)).dot" />
        </span>
        <div class="flex min-w-0 items-center gap-2">
          <AgentPortrait :who="agent" :size="PORTRAIT_SIZE" />
          <span class="min-w-0 flex-1 truncate text-control font-semibold text-ink">{{ roster.nameOf(agent) }}</span>
          <UiBadge :status="badgeOf(agent, activity.runsOf(agent))" :label="presence(agent, activity.runsOf(agent)).label" />
        </div>
        <template v-if="activity.runsOf(agent).length > 0">
          <p v-if="activity.runsOf(agent).length > 1" class="text-small text-muted">
            {{ ACTIVITY_COPY.copies(activity.runsOf(agent).length) }}
          </p>
          <p class="truncate text-small text-muted" :title="projectNames(activity.runsOf(agent)).join(', ')">
            {{ ACTIVITY_COPY.projects }}: {{ projectNames(activity.runsOf(agent)).join(', ') }}
          </p>
          <ul class="m-0 flex list-none flex-col gap-0.5 p-0">
            <li
              v-for="run in activity.runsOf(agent)"
              :key="run.instanceId + run.projectId"
              class="truncate text-small text-ink"
              :title="runSummary(run)"
            >
              {{ runSummary(run) }}
            </li>
          </ul>
        </template>
      </li>
    </ul>
  </section>
</template>
