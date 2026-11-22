<script setup lang="ts">
import { ref, watch } from 'vue'
import AgentPortrait from './AgentPortrait.vue'
import AgentProjectIcons from './AgentProjectIcons.vue'
import { PORTRAIT_SIZES } from './portraitArt'
import { useRoster } from './useRoster'
import { useConnection } from './useConnection'
import { useFeed } from './useFeed'
import { useRequests } from './useRequests'
import { presenceOf } from './agentStateStyle'
import { useChannels } from '../channels/useChannels'
import { GENERAL_CHANNEL_NAME } from '../channels/channelRouting'
import type { Channel } from '../channels/channelsApi'
import ChannelDialog from '../channels/ChannelDialog.vue'
import ChannelMenu from '../channels/ChannelMenu.vue'
import type { Agent } from './events'
import { useAgentControl } from '../panel/useAgentControl'
import { SAME_FOLDER_TEXT, worktreeFolder } from './agentCopies'
import { useAgentCopies } from './useAgentCopies'
import { useProjects } from '../projects/useProjects'
import UiCreateButton from '../../ui/UiCreateButton.vue'
import UiHandLine from '../../ui/UiHandLine.vue'
import UiNotice from '../../ui/UiNotice.vue'

const NEW_CHANNEL_LABEL = 'Novo canal'
const PANEL_CLASSES =
  'flex min-h-0 w-full shrink-0 flex-col overflow-y-auto rounded-dialog border border-line bg-surface p-2.5 max-md:max-h-72 md:w-52'
const QUEUE_BADGE_CLASSES =
  'absolute right-2 top-2 grid min-w-5 place-items-center rounded-full bg-accent-soft px-1.5 text-tiny font-semibold text-accent'
const MENTION_LABEL = 'Mencionar'
const OPEN_CONVERSATION_PREFIX = 'Abrir conversa de '
const CARD_CLASSES =
  'flex w-full cursor-pointer items-center gap-2 rounded-card border p-2 pr-8 text-left focus:outline-2 focus:outline-solid focus:outline-accent'
const MENTION_CLASSES =
  'absolute right-2 bottom-2 grid size-5 cursor-pointer place-items-center rounded-full text-tiny font-semibold text-muted transition-[transform,color,background-color] duration-150 ease-out enabled:hover:scale-115 enabled:hover:bg-accent-soft enabled:hover:text-accent focus-visible:scale-115 focus-visible:bg-accent-soft focus-visible:text-accent focus:outline-2 focus:outline-solid focus:outline-accent disabled:opacity-40 motion-reduce:transition-none'
const MENTION_GLYPH = '@'
const SELECTED_CLASSES = 'border-accent bg-accent-soft'
const UNSELECTED_CLASSES = 'border-line hover:bg-hover'
const CHANNEL_CLASSES =
  'flex w-full cursor-pointer items-center rounded-control border py-1.5 pl-3 pr-8 text-left text-label text-ink focus:outline-2 focus:outline-solid focus:outline-accent'
const COPY_CLASSES =
  'flex w-full cursor-pointer flex-col gap-0.5 rounded-control border border-line px-2.5 py-1.5 text-left focus:outline-2 focus:outline-solid focus:outline-accent'
const COPY_SELECTED_CLASSES = 'outline-2 outline-solid outline-accent bg-accent-soft'
const COPIES_LIST_LABEL = 'Cópias'
const COPY_MARK_TEXT = 'Cópia'
const COPY_NOUN_SINGULAR = 'cópia'
const COPY_NOUN_PLURAL = 'cópias'

const connection = useConnection()
const feed = useFeed()
const requests = useRequests()
const roster = useRoster()
const control = useAgentControl()
const channels = useChannels()
const agentCopies = useAgentCopies()
const projects = useProjects()
const creating = ref(false)
const editing = ref<Channel | null>(null)
// Abaixo de md a lista vira faixa no topo, recolhida até o usuário abri-la.
const stripCollapsed = ref(true)
const channelError = ref<string | null>(null)
const opener = ref<HTMLElement | null>(null)

// Cópias surgem e somem com os turnos: recarrega quando o projeto ou o estado de algum agente muda.
watch(
  [() => projects.selected?.id, () => feed.states],
  () => void agentCopies.load(),
  { immediate: true, deep: true },
)

function presence(agent: Agent) {
  return presenceOf(feed.stateOf(agent), control.isPaused(agent))
}

function copiesLabel(count: number): string {
  return `${count} ${count === 1 ? COPY_NOUN_SINGULAR : COPY_NOUN_PLURAL}`
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
      feed.showScreen({ kind: 'channel', channel: GENERAL_CHANNEL_NAME })
    }
  } catch (error) {
    channelError.value = String(error)
  }
}

function toggleStrip(): void {
  stripCollapsed.value = !stripCollapsed.value
}

function openChannelDialog(channel: Channel | null = null): void {
  editing.value = channel
  opener.value = document.activeElement instanceof HTMLElement ? document.activeElement : null
  creating.value = true
}

function closeChannelDialog(): void {
  creating.value = false
  editing.value = null
  opener.value?.focus()
}
</script>

<template>
  <aside :class="PANEL_CLASSES">
    <button
      type="button"
      class="mb-2 flex w-full items-center justify-between text-small font-semibold text-muted md:hidden"
      :aria-expanded="!stripCollapsed"
      @click="toggleStrip"
    >
      Canais e agentes
      <span aria-hidden="true">{{ stripCollapsed ? '▾' : '▴' }}</span>
    </button>
    <div :class="['flex min-h-0 flex-col', stripCollapsed && 'max-md:hidden']">
      <div class="mb-2 flex items-center justify-between">
        <h2 class="text-small font-semibold text-muted">Canais</h2>
        <UiCreateButton @press="openChannelDialog()">{{ NEW_CHANNEL_LABEL }}</UiCreateButton>
      </div>
      <ul class="mb-2 flex flex-col gap-2">
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
        <li v-for="channel in channels.channels" :key="channel.id" class="group relative">
          <button
            type="button"
            :class="[CHANNEL_CLASSES, channelShown(channel.id) ? SELECTED_CLASSES : UNSELECTED_CLASSES]"
            :aria-label="`Abrir canal #${channel.name}`"
            :aria-pressed="channelShown(channel.id)"
            @click="feed.showScreen({ kind: 'channel', channel: channel.id })"
          >
            #{{ channel.name }}
          </button>
          <ChannelMenu :name="channel.name" @edit="openChannelDialog(channel)" @remove="removeChannel(channel)" />
        </li>
      </ul>
      <UiNotice v-if="channelError" class="mb-3" :text="channelError" @close="channelError = null" />
      <UiHandLine class="mb-3 h-1.5 w-full text-line" />
      <h2 class="mb-2 text-small font-semibold text-muted">Agentes</h2>
      <ul class="flex flex-col gap-1.5">
        <li v-for="agent in roster.agents" :key="agent.id" class="relative">
          <button
            type="button"
            :class="[CARD_CLASSES, isShown(agent.id) ? SELECTED_CLASSES : UNSELECTED_CLASSES]"
            :aria-label="OPEN_CONVERSATION_PREFIX + agent.name"
            :aria-pressed="isShown(agent.id)"
            @click="feed.showScreen({ kind: 'agent', agent: agent.id })"
          >
            <span class="flex shrink-0" aria-hidden="true">
              <AgentPortrait :who="agent.id" :size="PORTRAIT_SIZES.stack" />
            </span>
            <span class="flex min-w-0 flex-col">
              <span class="flex min-w-0 items-center gap-1.5 text-label font-semibold text-ink">
                <span class="truncate">{{ agent.name }}</span>
                <AgentProjectIcons :agent="agent.id" />
              </span>
              <span class="flex min-w-0 items-center gap-1.5 text-tiny text-ink">
                <span class="size-1.5 shrink-0 rounded-full" :class="presence(agent.id).dot" />
                <span class="truncate">{{ presence(agent.id).label }}</span>
              </span>
              <span class="block truncate text-tiny text-muted">{{ agent.role }}</span>
            </span>
            <span v-if="agentCopies.copiesOf(agent.id).length" class="ml-auto shrink-0 self-start whitespace-nowrap text-tiny text-muted">
              {{ copiesLabel(agentCopies.copiesOf(agent.id).length) }}
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
          <ul
            v-if="agentCopies.copiesOf(agent.id).length"
            :aria-label="`${COPIES_LIST_LABEL} de ${agent.name}`"
            class="mt-1 ml-5 flex flex-col gap-1"
          >
            <li v-for="copy in agentCopies.copiesOf(agent.id)" :key="copy.id">
              <button
                type="button"
                :class="[COPY_CLASSES, agentCopies.selectedId === copy.id && COPY_SELECTED_CLASSES]"
                :aria-pressed="agentCopies.selectedId === copy.id"
                @click="agentCopies.select(copy.id)"
              >
                <span class="text-label font-semibold text-ink">{{ COPY_MARK_TEXT }} {{ copy.number }}</span>
                <span v-if="copy.itemId" class="truncate text-tiny text-muted">
                  {{ copy.itemId }}<template v-if="copy.itemTitle"> · {{ copy.itemTitle }}</template>
                </span>
                <span v-if="copy.worktreePath" class="truncate font-mono text-tiny text-muted">
                  {{ worktreeFolder(copy.worktreePath) }}
                </span>
                <span v-else class="truncate text-tiny text-muted">{{ SAME_FOLDER_TEXT }}</span>
              </button>
            </li>
          </ul>
        </li>
      </ul>
    </div>
  </aside>
  <ChannelDialog v-if="creating" :key="editing?.id" :channel="editing ?? undefined" @close="closeChannelDialog" />
</template>
