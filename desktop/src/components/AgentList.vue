<script setup lang="ts">
import { AGENTS, useTeam } from '../stores/useTeam'
import type { AgentState } from '../types/events'

const STATE_LABELS: Record<AgentState, string> = {
  thinking: 'pensando',
  waiting: 'esperando',
  idle: 'livre',
}

const DOT_CLASSES: Record<AgentState, string> = {
  thinking: 'animate-pulse bg-accent',
  waiting: 'bg-accent/50',
  idle: 'bg-ink/30',
}

const team = useTeam()
</script>

<template>
  <aside class="w-60 shrink-0 border-r border-line bg-white p-4">
    <h2 class="mb-3 text-xs font-semibold uppercase tracking-wide text-ink/50">Agentes</h2>
    <ul class="flex flex-col gap-2">
      <li v-for="agent in AGENTS" :key="agent.id" class="rounded-xl border border-line px-3 py-2">
        <p class="text-sm font-medium text-ink">{{ agent.name }}</p>
        <p class="text-xs text-ink/60">{{ agent.role }}</p>
        <p
          class="mt-1 flex items-center gap-1.5 text-xs"
          :class="team.states[agent.id] === 'idle' ? 'text-ink/50' : 'text-accent'"
        >
          <span class="h-1.5 w-1.5 rounded-full" :class="DOT_CLASSES[team.states[agent.id]]" />
          {{ STATE_LABELS[team.states[agent.id]] }}
        </p>
      </li>
    </ul>
  </aside>
</template>
