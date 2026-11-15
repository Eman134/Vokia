<script setup lang="ts">
import AgentPortrait from './AgentPortrait.vue'
import AgentProjectIcons from './AgentProjectIcons.vue'
import { PORTRAIT_SIZES } from './portraitArt'
import { useRoster } from './useRoster'
import { useConnection } from './useConnection'
import { useFeed } from './useFeed'
import { useRequests } from './useRequests'
import { presenceOf } from './agentStateStyle'
import type { Agent } from './events'
import { useAgentControl } from '../panel/useAgentControl'
import { useConsole } from '../console/useConsole'
import UiButton from '../../ui/UiButton.vue'

const CONSOLE_LABEL = 'Console'

const connection = useConnection()


const feed = useFeed()


const requests = useRequests()
const roster = useRoster()
const control = useAgentControl()
const consoleStore = useConsole()

function presence(agent: Agent) {
  return presenceOf(feed.stateOf(agent), control.isPaused(agent))
}
</script>

<template>
  <aside class="w-60 shrink-0 border-r border-line bg-surface p-4">
    <h2 class="mb-3 text-small font-semibold text-muted">Agentes</h2>
    <ul class="flex flex-col gap-2">
      <li v-for="agent in roster.agents" :key="agent.id">
        <button
          type="button"
          class="flex w-full cursor-pointer items-center gap-3 rounded-card border border-line p-3 text-left focus:outline-2 focus:outline-solid focus:outline-accent disabled:opacity-40"
          :aria-label="`Mencionar ${agent.name}`"
          :disabled="connection.checking"
          @click="requests.requestMention(agent.id)"
        >
          <span class="flex shrink-0" aria-hidden="true">
            <AgentPortrait :who="agent.id" :size="PORTRAIT_SIZES.list" />
          </span>
          <span class="flex min-w-0 flex-col">
            <span class="flex items-center gap-1.5 text-body font-semibold text-ink">
              {{ agent.name }}
              <AgentProjectIcons :agent="agent.id" />
            </span>
            <span class="block text-caption text-muted">{{ agent.role }}</span>
            <span class="mt-1 flex items-center gap-1.5 text-caption text-ink">
              <span class="size-1.75 rounded-full" :class="presence(agent.id).dot" />
              {{ presence(agent.id).label }}
            </span>
          </span>
        </button>
        <UiButton
          v-if="consoleStore.entriesOf(agent.id).length > 0"
          class="mt-1"
          variant="quiet"
          size="sm"
          :aria-label="`${CONSOLE_LABEL} de ${agent.name}`"
          @press="consoleStore.open(agent.id)"
        >
          {{ CONSOLE_LABEL }}
        </UiButton>
      </li>
    </ul>
  </aside>
</template>
