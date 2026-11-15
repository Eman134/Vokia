<script setup lang="ts">
import AgentPortrait from './AgentPortrait.vue'
import AgentProjectIcons from './AgentProjectIcons.vue'
import { PORTRAIT_SIZES } from './portraitArt'
import { useRoster } from './useRoster'
import { useConnection } from './useConnection'
import { useFeed } from './useFeed'
import { useRequests } from './useRequests'
import { presenceOf } from './agentStateStyle'
import { ALL_SCREEN, ALL_SCREEN_LABEL } from './timeline'
import type { Agent } from './events'
import { useAgentControl } from '../panel/useAgentControl'
import { useConsole } from '../console/useConsole'
import UiButton from '../../ui/UiButton.vue'

const CONSOLE_LABEL = 'Console'
const MENTION_LABEL = 'Mencionar'
const OPEN_CONVERSATION_PREFIX = 'Abrir conversa de '
const ALL_CONVERSATION_LABEL = 'Abrir conversa de todos'
const CARD_CLASSES =
  'flex w-full cursor-pointer items-center gap-3 rounded-card border p-3 pr-10 text-left focus:outline-2 focus:outline-solid focus:outline-accent'
const MENTION_CLASSES =
  'absolute right-3 top-3 grid size-6 cursor-pointer place-items-center rounded-full text-label font-semibold text-muted transition-[transform,color,background-color] duration-150 ease-out enabled:hover:scale-115 enabled:hover:bg-accent-soft enabled:hover:text-accent focus-visible:scale-115 focus-visible:bg-accent-soft focus-visible:text-accent focus:outline-2 focus:outline-solid focus:outline-accent disabled:opacity-40 motion-reduce:transition-none'
const MENTION_GLYPH = '@'
const SELECTED_CLASSES = 'border-accent bg-accent-soft'
const UNSELECTED_CLASSES = 'border-line'

const connection = useConnection()


const feed = useFeed()


const requests = useRequests()
const roster = useRoster()
const control = useAgentControl()
const consoleStore = useConsole()

function presence(agent: Agent) {
  return presenceOf(feed.stateOf(agent), control.isPaused(agent))
}

function isShown(agent: Agent): boolean {
  return feed.screen.kind === 'agent' && feed.screen.agent === agent
}
</script>

<template>
  <aside class="w-60 shrink-0 border-r border-line bg-surface p-4">
    <button
      type="button"
      class="mb-4 text-body font-semibold text-ink"
      :class="[CARD_CLASSES, feed.screen.kind === 'all' ? SELECTED_CLASSES : UNSELECTED_CLASSES]"
      :aria-label="ALL_CONVERSATION_LABEL"
      :aria-pressed="feed.screen.kind === 'all'"
      @click="feed.showScreen(ALL_SCREEN)"
    >
      {{ ALL_SCREEN_LABEL }}
    </button>
    <h2 class="mb-3 text-small font-semibold text-muted">Agentes</h2>
    <ul class="flex flex-col gap-2">
      <li v-for="agent in roster.agents" :key="agent.id" class="relative">
        <button
          type="button"
          :class="[CARD_CLASSES, isShown(agent.id) ? SELECTED_CLASSES : UNSELECTED_CLASSES]"
          :aria-label="OPEN_CONVERSATION_PREFIX + agent.name"
          :aria-pressed="isShown(agent.id)"
          @click="feed.showScreen({ kind: 'agent', agent: agent.id })"
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
        <button
          type="button"
          :class="MENTION_CLASSES"
          :aria-label="`${MENTION_LABEL} ${agent.name}`"
          :disabled="connection.checking"
          @click="requests.requestMention(agent.id)"
        >
          {{ MENTION_GLYPH }}
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
