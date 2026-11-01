<script setup lang="ts">
import AgentPortrait from './AgentPortrait.vue'
import { PORTRAIT_SIZES } from './portraitArt'
import { AGENTS, useTeam } from './useTeam'
import { presenceOf } from './agentStateStyle'
import type { Agent } from './events'
import { useAgentControl } from '../panel/useAgentControl'

const team = useTeam()
const control = useAgentControl()

function presence(agent: Agent) {
  return presenceOf(team.states[agent], control.isPaused(agent))
}
</script>

<template>
  <aside class="w-60 shrink-0 border-r border-line bg-surface p-4">
    <h2 class="mb-3 text-small font-semibold text-muted">Agentes</h2>
    <ul class="flex flex-col gap-2">
      <li v-for="agent in AGENTS" :key="agent.id">
        <button
          type="button"
          class="flex w-full cursor-pointer items-center gap-3 rounded-card border border-line p-3 text-left focus:outline-2 focus:outline-solid focus:outline-accent disabled:opacity-40"
          :aria-label="`Mencionar ${agent.name}`"
          :disabled="team.checking"
          @click="team.requestMention(agent.id)"
        >
          <span class="flex shrink-0" aria-hidden="true">
            <AgentPortrait :who="agent.id" :size="PORTRAIT_SIZES.list" />
          </span>
          <span class="flex min-w-0 flex-col">
            <span class="block text-body font-semibold text-ink">{{ agent.name }}</span>
            <span class="block text-caption text-muted">{{ agent.role }}</span>
            <span class="mt-1 flex items-center gap-1.5 text-caption text-ink">
              <span class="size-1.75 rounded-full" :class="presence(agent.id).dot" />
              {{ presence(agent.id).label }}
            </span>
          </span>
        </button>
      </li>
    </ul>
  </aside>
</template>
