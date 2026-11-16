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
import { useChannels } from '../channels/useChannels'
import { GENERAL_CHANNEL_NAME } from '../channels/channelRouting'
import type { Channel } from '../channels/channelsApi'
import ChannelForm from '../channels/ChannelForm.vue'
import type { Agent } from './events'
import { useAgentControl } from '../panel/useAgentControl'
import { ref } from 'vue'
import { useConsole } from '../console/useConsole'
import UiButton from '../../ui/UiButton.vue'
import UiConfirm from '../../ui/UiConfirm.vue'
import UiNotice from '../../ui/UiNotice.vue'

const CONSOLE_LABEL = 'Console'
const SEARCH_LABEL = 'Buscar conversas'
const PANEL_CLASSES =
  'flex min-h-0 w-59 shrink-0 flex-col overflow-y-auto rounded-dialog border border-line bg-surface p-2.5'
const SEARCH_CLASSES =
  'mb-3 h-8.5 w-full rounded-control bg-surface-2 px-3 text-label text-ink placeholder:text-muted disabled:opacity-40'
const QUEUE_BADGE_CLASSES =
  'absolute right-3 top-10 grid min-w-5 place-items-center rounded-full bg-accent-soft px-1.5 text-tiny font-semibold text-accent'
const MENTION_LABEL = 'Mencionar'
const OPEN_CONVERSATION_PREFIX = 'Abrir conversa de '
const ALL_CONVERSATION_LABEL = 'Abrir conversa de todos'
const CARD_CLASSES =
  'flex w-full cursor-pointer items-center gap-3 rounded-card border p-3 pr-10 text-left focus:outline-2 focus:outline-solid focus:outline-accent'
const MENTION_CLASSES =
  'absolute right-3 top-3 grid size-6 cursor-pointer place-items-center rounded-full text-label font-semibold text-muted transition-[transform,color,background-color] duration-150 ease-out enabled:hover:scale-115 enabled:hover:bg-accent-soft enabled:hover:text-accent focus-visible:scale-115 focus-visible:bg-accent-soft focus-visible:text-accent focus:outline-2 focus:outline-solid focus:outline-accent disabled:opacity-40 motion-reduce:transition-none'
const MENTION_GLYPH = '@'
const SELECTED_CLASSES = 'border-accent bg-accent-soft'
const CHANNEL_CLASSES =
  'flex w-full cursor-pointer items-center rounded-control border px-3 py-2 text-left text-label text-ink focus:outline-2 focus:outline-solid focus:outline-accent'
const DELETE_CHANNEL_MESSAGE = 'Apaga o canal. As mensagens dele voltam para #geral.'
const DELETE_CHANNEL_LABEL = 'Excluir canal'
const NEW_CHANNEL_LABEL = 'Novo canal'
const UNSELECTED_CLASSES = 'border-line'

const connection = useConnection()


const feed = useFeed()


const requests = useRequests()
const roster = useRoster()
const control = useAgentControl()
const consoleStore = useConsole()
const channels = useChannels()
const creating = ref(false)
const channelError = ref<string | null>(null)

function presence(agent: Agent) {
  return presenceOf(feed.stateOf(agent), control.isPaused(agent))
}

function isShown(agent: Agent): boolean {
  return feed.screen.kind === 'agent' && feed.screen.agent === agent
}

function channelShown(target: number | typeof GENERAL_CHANNEL_NAME): boolean {
  return feed.screen.kind === 'channel' && feed.screen.channel === target
}

async function removeChannel(channel: Channel): Promise<void> {
  channelError.value = null
  try {
    await channels.remove(channel.id)
    if (channelShown(channel.id)) {
      feed.showScreen(ALL_SCREEN)
    }
  } catch (error) {
    channelError.value = String(error)
  }
}
</script>

<template>
  <aside :class="PANEL_CLASSES">
    <input type="search" :class="SEARCH_CLASSES" :aria-label="SEARCH_LABEL" :placeholder="SEARCH_LABEL" disabled />
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
    <h2 class="mb-3 text-small font-semibold text-muted">Canais</h2>
    <ul class="mb-4 flex flex-col gap-2">
      <li>
        <button
          type="button"
          :class="[CHANNEL_CLASSES, channelShown(GENERAL_CHANNEL_NAME) ? SELECTED_CLASSES : UNSELECTED_CLASSES]"
          :aria-label="`Abrir canal #${GENERAL_CHANNEL_NAME}`"
          :aria-pressed="channelShown(GENERAL_CHANNEL_NAME)"
          @click="feed.showScreen({ kind: 'channel', channel: GENERAL_CHANNEL_NAME })"
        >
          #{{ GENERAL_CHANNEL_NAME }}
        </button>
      </li>
      <li v-for="channel in channels.channels" :key="channel.id" class="flex flex-col gap-1">
        <button
          type="button"
          :class="[CHANNEL_CLASSES, channelShown(channel.id) ? SELECTED_CLASSES : UNSELECTED_CLASSES]"
          :aria-label="`Abrir canal #${channel.name}`"
          :aria-pressed="channelShown(channel.id)"
          @click="feed.showScreen({ kind: 'channel', channel: channel.id })"
        >
          #{{ channel.name }}
        </button>
        <UiConfirm
          :label="`Excluir canal #${channel.name}`"
          :message="DELETE_CHANNEL_MESSAGE"
          :confirm-label="DELETE_CHANNEL_LABEL"
          @confirm="removeChannel(channel)"
        />
      </li>
    </ul>
    <UiButton variant="quiet" size="sm" class="mb-3" :aria-expanded="creating" @press="creating = !creating">
      {{ NEW_CHANNEL_LABEL }}
    </UiButton>
    <ChannelForm v-if="creating" class="mb-4" @done="creating = false" />
    <UiNotice v-if="channelError" class="mb-3" :text="channelError" @close="channelError = null" />
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
        <span
          v-if="requests.queuedFor(agent.id).length > 0"
          :class="QUEUE_BADGE_CLASSES"
          :title="`${requests.queuedFor(agent.id).length} na fila`"
        >
          {{ requests.queuedFor(agent.id).length }}
        </span>
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
