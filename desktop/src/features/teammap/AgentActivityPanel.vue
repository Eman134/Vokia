<script setup lang="ts">
import { watch } from 'vue'
import AgentPortrait from '../team/AgentPortrait.vue'
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
    <ul class="m-0 grid list-none grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-2 p-0">
      <li v-for="agent in agents" :key="agent" class="flex min-w-0 flex-col gap-1.5 rounded-card border border-line bg-surface p-2.5">
        <div class="flex min-w-0 items-center gap-2">
          <AgentPortrait :who="agent" :size="PORTRAIT_SIZE" />
          <span class="min-w-0 flex-1 truncate text-control font-semibold text-ink">{{ roster.nameOf(agent) }}</span>
          <span class="inline-flex items-center gap-1.5 text-small text-muted">
            <span class="size-2 rounded-full" :class="presence(agent, activity.runsOf(agent)).dot" aria-hidden="true" />
            {{ presence(agent, activity.runsOf(agent)).label }}
          </span>
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
