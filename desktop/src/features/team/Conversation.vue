<script lang="ts">
export const EMPTY_FEED_TITLE = 'Nenhuma mensagem ainda.'
export const EMPTY_FEED_TEXT = 'Peça a primeira na barra de ação.'
export const EMPTY_SCREEN_TITLE_PREFIX = 'Nenhuma mensagem com '
export const EMPTY_SCREEN_TITLE_SUFFIX = ' ainda.'
export const NO_PROJECT_TITLE = 'Nenhum projeto ainda.'
export const NO_PROJECT_TEXT = 'Crie um projeto para começar a conversar com o time.'
</script>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import type { VokiaMessage } from './events'
import { agentName } from './agents'
import { ALL_SCREEN_LABEL, groupParticipants } from './timeline'
import { useFeed } from './useFeed'
import MessageEntry from './MessageEntry.vue'
import PermissionPrompt from './PermissionPrompt.vue'
import AgentPortrait from './AgentPortrait.vue'
import { PORTRAIT_SIZES } from './portraitArt'
import { useRoster } from './useRoster'
import { useChannels } from '../channels/useChannels'
import { GENERAL_CHANNEL_NAME, namingChannel } from '../channels/channelRouting'
import VokiaMascot from '../brand/VokiaMascot.vue'
import { useProjects } from '../projects/useProjects'
import UiConfirm from '../../ui/UiConfirm.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { useFollowEnd } from '../../ui/useFollowEnd'
import ConsoleCard from '../console/ConsoleCard.vue'
import { useConsole } from '../console/useConsole'

const GROUP_SEPARATOR = ' ↔ '
const PANEL_CLASSES =
  'flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-dialog border border-line bg-surface min-h-[20rem] md:min-h-0 lg:flex-[1_1_420px]'
const HEADER_CLASSES = 'flex flex-wrap items-center justify-between gap-4 border-b border-line px-4.5 py-3.5'
const BODY_CLASSES = 'flex min-h-0 flex-1 flex-col overflow-y-auto px-4.5 py-4'
const TABS_LABEL = 'Abas da conversa'
const TABS_CLASSES = 'flex gap-0.5 rounded-control bg-surface-2 p-0.75'
const TAB_CLASSES = 'whitespace-nowrap rounded-control-sm px-3 py-1.25 text-caption focus:outline-2 focus:outline-solid focus:outline-accent'
const TAB_ON = 'bg-surface font-medium text-ink shadow-seg'
const TAB_OFF = 'cursor-pointer text-muted'
const TAB_DISABLED = 'cursor-default text-muted opacity-40'
const TAB_LABELS = { conversa: 'Conversa', terminal: 'Terminal', fila: 'Fila' } as const

const feed = useFeed()

const roster = useRoster()
const channels = useChannels()
const consoleStore = useConsole()
const props = defineProps<{ filaEmAba: boolean }>()
const aba = defineModel<'conversa' | 'fila'>('aba', { default: 'conversa' })
// Com a fila em aba (largura estreita), ela ocupa o corpo no lugar das mensagens.
const filaAberta = computed(() => props.filaEmAba && aba.value === 'fila')

function groupTitle(messages: readonly VokiaMessage[]): string {
  return groupParticipants(messages).map(agentName).join(GROUP_SEPARATOR)
}

function channelName(target: number | typeof GENERAL_CHANNEL_NAME): string | null {
  if (target === GENERAL_CHANNEL_NAME) {
    return GENERAL_CHANNEL_NAME
  }
  return channels.channels.find((channel) => channel.id === target)?.name ?? null
}

function groupLabel(messages: readonly VokiaMessage[]): string {
  const name = namingChannel(
    groupParticipants(messages),
    channels.channels,
    roster.agents.map((agent) => agent.id),
  )
  return name === null ? groupTitle(messages) : `#${name}`
}

const screenLabel = computed(() => {
  if (feed.screen.kind === 'agent') {
    return agentName(feed.screen.agent)
  }
  if (feed.screen.kind === 'channel') {
    const name = channelName(feed.screen.channel)
    return name === null ? ALL_SCREEN_LABEL : `#${name}`
  }
  return ALL_SCREEN_LABEL
})
const clearable = computed(
  () => feed.screen.kind === 'all' || (feed.screen.kind === 'channel' && feed.screen.channel === GENERAL_CHANNEL_NAME),
)
const screenRole = computed(() => (feed.screen.kind === 'agent' ? (roster.profileOf(feed.screen.agent)?.role ?? '') : ''))

const projectsStore = useProjects()
const scroller = ref<HTMLElement | null>(null)
const { onScroll, followEnd } = useFollowEnd(scroller)
const clearFailure = ref<string | null>(null)

function countLabel(count: number): string {
  return count === 1 ? '1 mensagem' : `${count} mensagens`
}

async function clearConversation(): Promise<void> {
  clearFailure.value = null
  try {
    await feed.clearFeed()
  } catch (error) {
    clearFailure.value = `Não foi possível limpar a conversa: ${String(error)}`
  }
}

watch(
  [
    () => feed.feed.length,
    () => feed.permissions.length,
    () => feed.thinkingAgent,
    () => feed.screen,
    () => (feed.thinkingAgent ? consoleStore.entriesOf(feed.thinkingAgent.id).length : 0),
  ],
  followEnd,
)
onMounted(followEnd)
</script>

<template>
  <section :class="PANEL_CLASSES">
    <header :class="HEADER_CLASSES">
      <div class="flex min-w-0 items-center gap-3">
        <AgentPortrait v-if="feed.screen.kind === 'agent'" :who="feed.screen.agent" :size="PORTRAIT_SIZES.list" />
        <div class="flex min-w-0 flex-col">
          <span class="truncate text-lead text-ink" data-testid="conversation-screen">{{ screenLabel }}</span>
          <span v-if="screenRole" class="truncate text-caption text-muted">{{ screenRole }}</span>
        </div>
      </div>
      <div class="flex shrink-0 items-center gap-3">
        <div role="tablist" :aria-label="TABS_LABEL" :class="TABS_CLASSES">
          <button
            type="button"
            role="tab"
            :aria-selected="aba === 'conversa'"
            :class="[TAB_CLASSES, aba === 'conversa' ? TAB_ON : TAB_OFF]"
            @click="aba = 'conversa'"
          >
            {{ TAB_LABELS.conversa }}
          </button>
          <button type="button" role="tab" aria-disabled="true" :class="[TAB_CLASSES, TAB_DISABLED]">
            {{ TAB_LABELS.terminal }}
          </button>
          <button
            v-if="filaEmAba"
            type="button"
            role="tab"
            :aria-selected="aba === 'fila'"
            :class="[TAB_CLASSES, aba === 'fila' ? TAB_ON : TAB_OFF]"
            @click="aba = 'fila'"
          >
            {{ TAB_LABELS.fila }}
          </button>
        </div>
        <UiConfirm
          v-if="clearable && feed.feed.length > 0"
          label="Limpar"
          message="Apaga todas as mensagens desta conversa. O agente recomeça sem esse histórico."
          confirm-label="Limpar conversa"
          @confirm="clearConversation"
        />
      </div>
    </header>
    <section ref="scroller" :class="BODY_CLASSES" @scroll="onScroll">
    <template v-if="!filaAberta">
      <UiNotice v-if="clearFailure" class="mb-6" :text="clearFailure" @close="clearFailure = null" />
      <UiEmpty v-if="projectsStore.loaded && projectsStore.selected === null" class="m-auto" :title="NO_PROJECT_TITLE" :text="NO_PROJECT_TEXT">
        <template #mascot>
          <VokiaMascot look="tudo-em-dia" />
        </template>
      </UiEmpty>
      <UiEmpty v-else-if="feed.feed.length === 0" class="m-auto" :title="EMPTY_FEED_TITLE" :text="EMPTY_FEED_TEXT">
        <template #mascot>
          <VokiaMascot look="tudo-em-dia" />
        </template>
      </UiEmpty>
      <UiEmpty
        v-else-if="feed.timeline.length === 0"
        class="m-auto"
        :title="EMPTY_SCREEN_TITLE_PREFIX + screenLabel + EMPTY_SCREEN_TITLE_SUFFIX"
        :text="EMPTY_FEED_TEXT"
      >
        <template #mascot>
          <VokiaMascot look="tudo-em-dia" />
        </template>
      </UiEmpty>
      <ol class="flex max-w-3xl flex-col gap-3">
        <li v-for="item in feed.timeline" :key="item.start">
          <details v-if="item.type === 'group'" class="rounded-card border border-line bg-surface px-4 py-3">
            <summary class="flex min-w-0 cursor-pointer list-none items-baseline gap-2">
              <span class="min-w-0 truncate text-caption font-semibold text-ink" :title="groupTitle(item.messages)">{{ groupLabel(item.messages) }}</span>
              <span class="text-caption text-muted">{{ countLabel(item.messages.length) }}</span>
            </summary>
            <ol class="mt-3 flex flex-col gap-3">
              <li v-for="(message, index) in item.messages" :key="index">
                <MessageEntry :entry="message" />
              </li>
            </ol>
          </details>
          <MessageEntry v-else :entry="item.entry" />
        </li>
      </ol>
      <ol v-if="feed.permissions.length > 0" class="mt-3 flex max-w-3xl flex-col gap-3">
        <li v-for="permission in feed.permissions" :key="permission.id">
          <PermissionPrompt :permission="permission" @decide="(allow) => feed.decidePermission(permission.id, allow)" />
        </li>
      </ol>
      <div
        role="status"
        :aria-label="feed.thinkingAgent ? `${feed.thinkingAgent.name} está pensando` : undefined"
        class="mt-3 flex max-w-3xl items-start gap-2.5"
      >
        <template v-if="feed.thinkingAgent">
          <span
            class="grid size-7 shrink-0 place-items-center rounded-full border border-line bg-surface-2 text-small font-semibold text-ink"
          >
            {{ feed.thinkingAgent.name.charAt(0) }}
          </span>
          <ConsoleCard :agent="feed.thinkingAgent" />
        </template>
      </div>
    </template>
    <slot v-else name="fila" />
    </section>
  </section>
</template>
