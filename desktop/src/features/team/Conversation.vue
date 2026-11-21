<script lang="ts">
export const EMPTY_FEED_TITLE = 'Nenhuma mensagem ainda.'
export const EMPTY_FEED_TEXT = 'Peça a primeira na barra de ação.'
export const EMPTY_SCREEN_TITLE_PREFIX = 'Nenhuma mensagem com '
export const EMPTY_SCREEN_TITLE_SUFFIX = ' ainda.'
export const NO_PROJECT_TITLE = 'Nenhum projeto ainda.'
export const NO_PROJECT_TEXT = 'Crie um projeto para começar a conversar com o time.'
</script>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import type { VokiaMessage } from './events'
import { useConversationTab } from './conversationTabs'
import { agentName } from './agents'
import { ALL_SCREEN_LABEL, collapsesGroup, groupParticipants } from './timeline'
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
import UiButton from '../../ui/UiButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { useFollowEnd } from '../../ui/useFollowEnd'
import AgentTerminal from '../console/AgentTerminal.vue'
import ActivityRun from './ActivityRun.vue'
import AgentThinking from '../console/AgentThinking.vue'
import { useConsole } from '../console/useConsole'
import { STATE_LABELS } from './agentStateStyle'

const GROUP_SEPARATOR = ' ↔ '
const PANEL_CLASSES =
  'flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-dialog border border-line bg-surface min-h-[20rem] md:min-h-0 lg:flex-[1_1_420px]'
const HEADER_CLASSES = 'flex flex-wrap items-center justify-between gap-4 border-b border-line px-4.5 py-3.5'
const BODY_CLASSES = 'flex min-h-0 flex-1 flex-col overflow-y-auto px-4.5 py-4'
const TABS_LABEL = 'Abas da conversa'
const TABS_CLASSES = 'flex gap-5'
const TAB_CLASSES = 'cursor-pointer whitespace-nowrap border-b-2 pb-1 text-label font-medium focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-accent'
const TAB_ON = 'border-ink text-ink'
const TAB_OFF = 'border-transparent text-muted hover:text-ink'
const TAB_LABELS = { conversa: 'Conversa', terminal: 'Terminal', fila: 'Fila' } as const
const CLEAR_CONVERSATION_LABEL = 'Limpar conversa'
const CLEAR_CONVERSATION_TITLE = 'Limpar conversa?'
const CLEAR_CONVERSATION_MESSAGE = 'Apaga todas as mensagens desta conversa.'
const CLEAR_BUTTON_CLASSES =
  'flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-md text-muted transition-colors hover:bg-surface-2 hover:text-ink focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-accent'
const CLEAR_POPUP_CLASSES =
  'absolute right-0 top-full z-30 mt-1.5 flex w-64 flex-col gap-2.5 rounded-lg border border-line bg-surface p-3 shadow-float'

const feed = useFeed()

const roster = useRoster()
const channels = useChannels()
const consoleStore = useConsole()
const props = defineProps<{ filaEmAba: boolean }>()
const { tab: aba } = storeToRefs(useConversationTab())
// Com a fila em aba (largura estreita), ela ocupa o corpo no lugar das mensagens.
const filaAberta = computed(() => props.filaEmAba && aba.value === 'fila')
// Canal e "Todos" não têm terminal: só a conversa com um agente tem.
const temTerminal = computed(() => feed.screen.kind === 'agent')

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
const screenRole = computed(() => (feed.screen.kind === 'agent' ? (roster.profileOf(feed.screen.agent)?.role ?? '') : ''))

const projectsStore = useProjects()
const liveRun = computed(() =>
  feed.pendingAgent ? feed.liveActivityOf(feed.pendingAgent.id) : undefined,
)
const pendingLabel = computed(() =>
  feed.pendingAgent ? `${feed.pendingAgent.name} está ${STATE_LABELS[feed.stateOf(feed.pendingAgent.id)]}` : undefined,
)
const scroller = ref<HTMLElement | null>(null)
const { onScroll, followEnd } = useFollowEnd(scroller)
const clearFailure = ref<string | null>(null)
const confirmingClear = ref(false)
const clearWrap = ref<HTMLElement | null>(null)

function onClearKeydown(event: KeyboardEvent): void {
  if (confirmingClear.value && event.key === 'Escape' && !event.defaultPrevented) {
    event.preventDefault()
    confirmingClear.value = false
  }
}

function onClearPointerDown(event: PointerEvent): void {
  if (!confirmingClear.value) return
  if (event.target instanceof Node && !clearWrap.value?.contains(event.target)) {
    confirmingClear.value = false
  }
}

function countLabel(count: number): string {
  return count === 1 ? '1 mensagem' : `${count} mensagens`
}

async function clearConversation(): Promise<void> {
  confirmingClear.value = false
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
    () => feed.pendingAgent,
    () => liveRun.value?.tools.length,
    () => feed.screen,
    () => (feed.thinkingAgent ? consoleStore.entriesOf(feed.thinkingAgent.id).length : 0),
  ],
  followEnd,
)
onMounted(() => {
  followEnd()
  window.addEventListener('keydown', onClearKeydown)
  document.addEventListener('pointerdown', onClearPointerDown)
})
onUnmounted(() => {
  window.removeEventListener('keydown', onClearKeydown)
  document.removeEventListener('pointerdown', onClearPointerDown)
})
watch(
  () => feed.screen,
  () => {
    confirmingClear.value = false
  },
)
watch(temTerminal, (disponivel) => {
  if (!disponivel && aba.value === 'terminal') aba.value = 'conversa'
}, { flush: 'sync' })
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
          <button
            v-if="temTerminal"
            type="button"
            role="tab"
            :aria-selected="aba === 'terminal'"
            :class="[TAB_CLASSES, aba === 'terminal' ? TAB_ON : TAB_OFF]"
            @click="aba = 'terminal'"
          >
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
        <div v-if="feed.feed.length > 0" ref="clearWrap" class="relative flex items-center">
          <button
            type="button"
            :class="CLEAR_BUTTON_CLASSES"
            :title="CLEAR_CONVERSATION_LABEL"
            :aria-label="CLEAR_CONVERSATION_LABEL"
            :aria-expanded="confirmingClear"
            data-testid="confirm-open"
            @click="confirmingClear = !confirmingClear"
          >
            <svg
              class="size-4"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.6"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <path d="M3 6h18" />
              <path d="M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2" />
              <path d="M19 6v13a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
              <path d="M10 11v6" />
              <path d="M14 11v6" />
            </svg>
          </button>
          <div
            v-if="confirmingClear"
            role="dialog"
            :aria-label="CLEAR_CONVERSATION_LABEL"
            :class="CLEAR_POPUP_CLASSES"
          >
            <div class="flex flex-col gap-0.5">
              <p class="text-caption font-semibold text-ink">{{ CLEAR_CONVERSATION_TITLE }}</p>
              <p class="text-xs text-muted">{{ CLEAR_CONVERSATION_MESSAGE }}</p>
            </div>
            <div class="flex items-center justify-end gap-2">
              <UiButton variant="quiet" size="sm" data-testid="confirm-cancel" @press="confirmingClear = false">
                Cancelar
              </UiButton>
              <UiButton variant="danger" size="sm" data-testid="confirm-accept" @press="clearConversation">
                Limpar
              </UiButton>
            </div>
          </div>
        </div>
      </div>
    </header>
    <section v-show="aba !== 'terminal'" ref="scroller" :class="BODY_CLASSES" @scroll="onScroll">
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
          <details
            v-if="item.type === 'group' && collapsesGroup(item.messages)"
            class="rounded-card border border-line bg-surface px-4 py-3"
          >
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
          <div v-else-if="item.type === 'group'" class="flex flex-col gap-3">
            <MessageEntry v-for="(message, index) in item.messages" :key="index" :entry="message" />
          </div>
          <MessageEntry v-else :entry="item.entry" />
        </li>
      </ol>
      <ol v-if="feed.permissions.length > 0" class="mt-3 flex max-w-3xl flex-col gap-3">
        <li v-for="permission in feed.permissions" :key="permission.id">
          <PermissionPrompt :permission="permission" @decide="(allow, reason) => feed.decidePermission(permission.id, allow, false, reason)" />
        </li>
      </ol>
      <div v-if="liveRun" class="mt-3 flex max-w-3xl items-start gap-2.5" data-testid="activity-live-block">
        <span class="size-7 shrink-0" aria-hidden="true" />
        <ActivityRun :activity="liveRun" live class="min-w-0 flex-1" />
      </div>
      <div
        role="status"
        :aria-label="pendingLabel"
        class="mt-3 flex max-w-3xl items-start gap-2.5"
      >
        <template v-if="feed.pendingAgent">
          <span class="relative flex shrink-0" aria-hidden="true">
            <AgentPortrait :who="feed.pendingAgent.id" :size="PORTRAIT_SIZES.chat" />
            <span class="absolute -right-0.5 -bottom-0.5 size-2 rounded-full bg-accent motion-reduce:animate-none animate-pulse" />
          </span>
          <AgentThinking
            :agent="feed.pendingAgent"
            :state="feed.stateOf(feed.pendingAgent.id)"
            :since="feed.pendingSince"
          />
        </template>
      </div>
    </template>
    <slot v-else name="fila" />
    </section>
    <AgentTerminal v-if="aba === 'terminal'" @answer="aba = 'conversa'" @exit="aba = 'conversa'" />
  </section>
</template>
