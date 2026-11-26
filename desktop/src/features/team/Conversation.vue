<script lang="ts">
export const EMPTY_FEED_TITLE = 'Nenhuma mensagem ainda.'
export const EMPTY_SCREEN_TITLE_PREFIX = 'Nenhuma mensagem com '
export const EMPTY_SCREEN_TITLE_SUFFIX = ' ainda.'
export const NO_PROJECT_TITLE = 'Nenhum projeto ainda.'
export const NO_PROJECT_TEXT = 'Crie um projeto para começar a conversar com o time.'
</script>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import { useNavigation, type PendingMessageTarget } from '../../shell/useNavigation'
import type { TurnActivity } from './events'
import { useConversationTab } from './conversationTabs'
import { agentName } from './agents'
import { ALL_SCREEN, ALL_SCREEN_LABEL, type FeedEntry } from './timeline'
import { useFeed } from './useFeed'
import type { AgentProfile } from './teamApi'
import {
  filterPendingAgentsForScreen,
  formatPendingStatusLabel,
  groupPendingAgentsByState,
  isPendingState,
  type PendingAgentEntry,
} from './pendingAgents'
import MessageEntry from './MessageEntry.vue'
import AutoPermissionGroup from './AutoPermissionGroup.vue'
import PermissionRow from './PermissionRow.vue'
import AgentPortrait from './AgentPortrait.vue'
import { PORTRAIT_SIZES } from './portraitArt'
import { useRoster } from './useRoster'
import { useChannels } from '../channels/useChannels'
import { useThreads } from './useThreads'
import { GENERAL_CHANNEL_NAME, TRIAGE_CHANNEL_NAME } from '../channels/channelRouting'
import { useBacklog } from '../backlog/useBacklog'
import VokiaMascot from '../brand/VokiaMascot.vue'
import { useProjects } from '../projects/useProjects'
import UiButton from '../../ui/UiButton.vue'
import UiHandLine from '../../ui/UiHandLine.vue'
import { HAND_MARK_CLASSES, INK_FOCUS_CLASSES, handReveal } from '../../ui/handDrawn'
import UiEmpty from '../../ui/UiEmpty.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { HIDDEN_SCROLLBAR_CLASSES } from '../../ui/pageLayout'
import { useFollowEnd } from '../../ui/useFollowEnd'
import ConversationComposer from './ConversationComposer.vue'
import TriageChannelPanel from './TriageChannelPanel.vue'
import AgentThinking from '../console/AgentThinking.vue'
import { useConsole } from '../console/useConsole'
import { useTurnPlans } from './useTurnPlans'

const STATUS_GROUP_LIST_CLASSES = 'mt-3 flex w-full min-w-0 flex-col gap-3'
const LIVE_BLOCK_CLASSES = 'flex min-w-0 items-start gap-2'
const STATUS_GROUP_CLASSES = 'flex items-start gap-2.5'
const STATUS_PORTRAITS_CLASSES = 'flex shrink-0 items-center gap-1.5'
const STATUS_DOT_CLASSES = 'absolute -right-0.5 -bottom-0.5 size-2 rounded-full bg-accent motion-reduce:animate-none animate-pulse'
const PANEL_CLASSES =
  'flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-paper min-h-[20rem] md:min-h-0 lg:flex-[1_1_420px]'
// Altura fixa: conversa de agente (retrato, nome e papel) e canal ocupam o mesmo cabeçalho.
const HEADER_CLASSES = 'flex h-12 shrink-0 items-center justify-between gap-3 border-b border-line px-3 md:h-14 md:gap-4 md:px-4.5'
const BODY_CLASSES = `flex min-h-0 flex-1 flex-col overflow-y-auto px-3 py-2.5 md:px-4 md:py-3 ${HIDDEN_SCROLLBAR_CLASSES}`
const TABS_LABEL = 'Abas da conversa'
const TABS_CLASSES = 'flex gap-5'
const TAB_CLASSES = `group relative cursor-pointer whitespace-nowrap pb-1.5 text-label font-medium ${INK_FOCUS_CLASSES}`
const TAB_ON = 'text-ink'
const TAB_OFF = 'text-muted hover:text-ink'
const TAB_MARK = `${HAND_MARK_CLASSES} -bottom-0.5 left-0 w-full`
const TAB_LABELS = { conversa: 'Conversa', fila: 'Fila' } as const
const TRIAGE_ROLE_NONE = 'Nenhum card em triagem'

function triageRole(count: number): string {
  if (count === 0) return TRIAGE_ROLE_NONE
  return `${count} ${count === 1 ? 'card' : 'cards'} em triagem`
}
const CLEAR_CONVERSATION_LABEL = 'Limpar conversa'
const CLEAR_CONVERSATION_TITLE = 'Limpar conversa?'
const CLEAR_CONVERSATION_MESSAGE = 'Apaga todas as mensagens desta conversa.'
const CLEAR_BUTTON_CLASSES = `flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-md text-muted transition-colors hover:bg-surface-2 hover:text-ink ${INK_FOCUS_CLASSES}`
const CLEAR_POPUP_CLASSES =
  'absolute right-0 top-full z-30 mt-1.5 flex w-64 flex-col gap-2.5 rounded-lg border border-line bg-surface p-3 shadow-paper'
const HIGHLIGHT_MESSAGE_CLASSES = 'rounded-md bg-accent-soft/70 ring-1 ring-accent transition-colors'
const HIGHLIGHT_DURATION_MS = 2200

const feed = useFeed()
const navigation = useNavigation()

const roster = useRoster()
const channels = useChannels()
const consoleStore = useConsole()
const turnPlans = useTurnPlans()
const props = defineProps<{ filaEmAba: boolean }>()
const { tab: aba } = storeToRefs(useConversationTab())
// Com a fila em aba (largura estreita), ela ocupa o corpo no lugar das mensagens.
const filaAberta = computed(() => props.filaEmAba && aba.value === 'fila')

function channelName(target: number | typeof GENERAL_CHANNEL_NAME): string | null {
  if (target === GENERAL_CHANNEL_NAME) {
    return GENERAL_CHANNEL_NAME
  }
  return channels.channels.find((channel) => channel.id === target)?.name ?? null
}

const threads = useThreads()
const backlog = useBacklog()
const activeThread = computed(() => (feed.screen.kind === 'thread' ? threads.find(feed.screen.itemId) : undefined))

const screenLabel = computed(() => {
  if (feed.screen.kind === 'agent') {
    return agentName(feed.screen.agent)
  }
  if (feed.screen.kind === 'copy') {
    return `${agentName(feed.screen.baseAgent)} · Cópia ${feed.screen.number}`
  }
  if (feed.screen.kind === 'channel') {
    const name = channelName(feed.screen.channel)
    return name === null ? ALL_SCREEN_LABEL : `#${name}`
  }
  if (feed.screen.kind === 'thread') {
    return activeThread.value?.title ?? 'Thread'
  }
  if (feed.screen.kind === 'triage') {
    return `#${TRIAGE_CHANNEL_NAME}`
  }
  return ALL_SCREEN_LABEL
})
const screenRole = computed(() => {
  if (feed.screen.kind === 'agent') return roster.profileOf(feed.screen.agent)?.role ?? ''
  if (feed.screen.kind === 'copy') {
    if (feed.screen.itemId) {
      return feed.screen.itemTitle ? `${feed.screen.itemId} · ${feed.screen.itemTitle}` : feed.screen.itemId
    }
    return roster.profileOf(feed.screen.baseAgent)?.role ?? ''
  }
  if (feed.screen.kind === 'thread') return activeThread.value?.column ?? ''
  if (feed.screen.kind === 'triage') return triageRole(backlog.triagingIds.size)
  return ''
})

const projectsStore = useProjects()
const scopedPendingAgents = computed<AgentProfile[]>(() => {
  if (feed.screen.kind === 'copy') {
    const { copyId, baseAgent } = feed.screen
    if (!isPendingState(feed.stateOf(copyId))) return []
    const base = roster.profileOf(baseAgent)
    const fallback: AgentProfile = {
      id: copyId,
      name: screenLabel.value,
      role: screenRole.value,
      avatar: base?.avatar ?? baseAgent,
      cli: base?.cli ?? '',
      model: base?.model ?? '',
      command: base?.command ?? '',
      builtin: base?.builtin ?? false,
      extraInstructions: base?.extraInstructions ?? '',
      followDefaultLlm: base?.followDefaultLlm ?? true,
      fallback: base?.fallback ?? [],
    }
    return [base ? { ...base, id: copyId, name: screenLabel.value, role: screenRole.value } : fallback]
  }
  return filterPendingAgentsForScreen(feed.pendingAgents, feed.screen, {
    channels: channels.channels,
    thread: activeThread.value,
    agentItems: feed.stateItem,
    triagingIds: backlog.triagingIds,
  })
})
const pendingGroups = computed(() =>
  groupPendingAgentsByState(scopedPendingAgents.value, feed.states, feed.stateSince),
)
const activeThreadItemId = computed(() => (feed.screen.kind === 'thread' ? feed.screen.itemId : undefined))
const scopedLiveRuns = computed(() =>
  pendingGroups.value
    .flatMap((group) => group.entries)
    .map((entry) => ({ entry, activity: feed.liveActivityOf(entry.agent.id, activeThreadItemId.value) }))
    .filter((item): item is { entry: PendingAgentEntry; activity: TurnActivity } => item.activity !== undefined),
)
const quietGroups = computed(() => {
  const liveIds = new Set(scopedLiveRuns.value.map((run) => run.entry.agent.id))
  return pendingGroups.value
    .map((group) => ({ ...group, entries: group.entries.filter((entry) => !liveIds.has(entry.agent.id)) }))
    .filter((group) => group.entries.length > 0)
})
const pendingLabel = computed(() => formatPendingStatusLabel(pendingGroups.value))
const scroller = ref<HTMLElement | null>(null)
const { onScroll, followEnd } = useFollowEnd(scroller)
const clearFailure = ref<string | null>(null)
const confirmingClear = ref(false)
const clearWrap = ref<HTMLElement | null>(null)
const highlightedMessageKey = ref<string | null>(null)
let highlightTimer: ReturnType<typeof setTimeout> | null = null

function messageKeyOf(entry: FeedEntry): string | undefined {
  if (entry.type !== 'message') return undefined
  return `${entry.time}:${entry.from}`
}

function focusMessageTarget(target: PendingMessageTarget): boolean {
  const key = `${target.time}:${target.from}`
  const exists = feed.feed.some((entry) => entry.type === 'message' && messageKeyOf(entry) === key)
  if (!exists) return false

  if (target.itemId) {
    feed.showScreen({ kind: 'thread', itemId: target.itemId })
  } else if (target.channelId != null) {
    feed.showScreen({ kind: 'channel', channel: target.channelId })
  } else {
    feed.showScreen({ kind: 'channel', channel: GENERAL_CHANNEL_NAME })
  }

  void nextTick(() => {
    let element = scroller.value?.querySelector<HTMLElement>(`[data-message-key="${CSS.escape(key)}"]`)
    if (!element) {
      feed.showScreen(ALL_SCREEN)
      void nextTick(() => {
        const fallbackEl = scroller.value?.querySelector<HTMLElement>(`[data-message-key="${CSS.escape(key)}"]`)
        fallbackEl?.scrollIntoView({ behavior: 'smooth', block: 'center' })
      })
    } else {
      element.scrollIntoView({ behavior: 'smooth', block: 'center' })
    }
  })

  highlightedMessageKey.value = key
  if (highlightTimer !== null) clearTimeout(highlightTimer)
  highlightTimer = setTimeout(() => {
    highlightedMessageKey.value = null
    highlightTimer = null
  }, HIGHLIGHT_DURATION_MS)
  return true
}

function tryConsumePendingMessage(): void {
  const target = navigation.pendingMessageTarget
  if (!target) return
  if (focusMessageTarget(target)) {
    navigation.takePendingMessageTarget()
  }
}

watch([() => navigation.pendingMessageTarget, () => feed.feed.length], tryConsumePendingMessage, {
  immediate: true,
})

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
    () => scopedPendingAgents.value.map((agent) => agent.id).join(','),
    () => scopedLiveRuns.value.reduce((total, run) => total + run.activity.tools.length, 0),
    () => feed.screen,
    () =>
      scopedPendingAgents.value.reduce(
        (total, agent) => total + (feed.stateOf(agent.id) === 'thinking' ? consoleStore.entriesOf(agent.id).length : 0),
        0,
      ),
    () =>
      scopedPendingAgents.value
        .map((agent) => turnPlans.livePlanOf(agent.id, activeThreadItemId.value)?.updatedAt ?? 0)
        .join(','),
  ],
  followEnd,
)
onMounted(() => {
  followEnd()
  window.addEventListener('keydown', onClearKeydown)
  document.addEventListener('pointerdown', onClearPointerDown)
})
onUnmounted(() => {
  if (highlightTimer !== null) clearTimeout(highlightTimer)
  window.removeEventListener('keydown', onClearKeydown)
  document.removeEventListener('pointerdown', onClearPointerDown)
})
watch(
  () => feed.screen,
  () => {
    confirmingClear.value = false
  },
)
</script>

<template>
  <section :class="PANEL_CLASSES">
    <header :class="HEADER_CLASSES">
      <div class="flex min-w-0 items-center gap-3">
        <AgentPortrait
          v-if="feed.screen.kind === 'agent' || feed.screen.kind === 'copy'"
          :who="feed.screen.kind === 'copy' ? feed.screen.baseAgent : feed.screen.agent"
          :size="PORTRAIT_SIZES.chat"
        />
        <div class="flex min-w-0 items-baseline gap-2">
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
            <UiHandLine :class="[TAB_MARK, handReveal(aba === 'conversa')]" />
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
            <UiHandLine :class="[TAB_MARK, handReveal(aba === 'fila')]" />
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
    <section ref="scroller" :class="BODY_CLASSES" @scroll="onScroll">
    <template v-if="!filaAberta">
      <UiNotice v-if="clearFailure" class="mb-6" :text="clearFailure" @close="clearFailure = null" />
      <TriageChannelPanel v-if="feed.screen.kind === 'triage'" />
      <UiEmpty v-if="projectsStore.loaded && projectsStore.selected === null" class="m-auto" :title="NO_PROJECT_TITLE" :text="NO_PROJECT_TEXT">
        <template #mascot>
          <VokiaMascot look="tudo-em-dia" />
        </template>
      </UiEmpty>
      <UiEmpty v-else-if="feed.feed.length === 0" class="m-auto" :title="EMPTY_FEED_TITLE">
        <template #mascot>
          <VokiaMascot look="tudo-em-dia" />
        </template>
      </UiEmpty>
      <UiEmpty
        v-else-if="feed.timeline.length === 0"
        class="m-auto"
        :title="EMPTY_SCREEN_TITLE_PREFIX + screenLabel + EMPTY_SCREEN_TITLE_SUFFIX"
      >
        <template #mascot>
          <VokiaMascot look="tudo-em-dia" />
        </template>
      </UiEmpty>
      <ol class="flex w-full min-w-0 flex-col gap-2">
        <li v-for="item in feed.timeline" :key="item.start">
          <div v-if="item.type === 'group'" class="flex flex-col gap-2">
            <div
              v-for="(message, index) in item.messages"
              :key="index"
              :data-message-key="messageKeyOf(message)"
              :class="highlightedMessageKey === messageKeyOf(message) ? HIGHLIGHT_MESSAGE_CLASSES : undefined"
            >
              <MessageEntry :entry="message" />
            </div>
          </div>
          <AutoPermissionGroup v-else-if="item.type === 'permissions'" :agent="item.agent" :results="item.results" />
          <div
            v-else
            :data-message-key="messageKeyOf(item.entry)"
            :class="highlightedMessageKey === messageKeyOf(item.entry) ? HIGHLIGHT_MESSAGE_CLASSES : undefined"
          >
            <MessageEntry :entry="item.entry" />
          </div>
        </li>
        <li v-if="feed.permissions[0]" :key="feed.permissions[0].id">
          <PermissionRow :permission="feed.permissions[0]" :pending-count="feed.permissions.length" />
        </li>
      </ol>
      <div
        role="status"
        :aria-label="pendingLabel"
        :class="STATUS_GROUP_LIST_CLASSES"
      >
        <div
          v-for="run in scopedLiveRuns"
          :key="run.entry.agent.id"
          :class="LIVE_BLOCK_CLASSES"
          data-testid="activity-live-block"
        >
          <span class="relative mt-0.5 flex shrink-0" aria-hidden="true">
            <AgentPortrait :who="run.entry.agent.id" :size="PORTRAIT_SIZES.chat" />
            <span :class="STATUS_DOT_CLASSES" />
          </span>
          <AgentThinking
            :agent="run.entry.agent"
            :state="run.entry.state"
            :since="run.entry.since"
            :activity="run.activity"
            :item-id="activeThreadItemId"
          />
        </div>
        <div
          v-for="group in quietGroups"
          :key="group.state"
          :class="STATUS_GROUP_CLASSES"
        >
          <div :class="STATUS_PORTRAITS_CLASSES" aria-hidden="true">
            <span v-for="entry in group.entries" :key="entry.agent.id" class="relative flex shrink-0">
              <AgentPortrait :who="entry.agent.id" :size="PORTRAIT_SIZES.chat" />
              <span :class="STATUS_DOT_CLASSES" />
            </span>
          </div>
          <div class="flex min-w-0 flex-1 flex-col gap-2">
            <AgentThinking
              v-for="entry in group.entries"
              :key="entry.agent.id"
              :agent="entry.agent"
              :state="entry.state"
              :since="entry.since"
              :item-id="activeThreadItemId"
            />
          </div>
        </div>
      </div>
    </template>
    <slot v-else name="fila" />
    </section>
    <ConversationComposer v-if="!filaAberta && projectsStore.selected !== null" />
  </section>
</template>
