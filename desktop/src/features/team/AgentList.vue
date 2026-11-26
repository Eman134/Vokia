<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import AgentPortrait from './AgentPortrait.vue'
import AgentProjectIcons from './AgentProjectIcons.vue'
import AgentPicker from '../agents/AgentPicker.vue'
import { PORTRAIT_SIZES } from './portraitArt'
import { categorizeThreads, type ThreadCategoryKey, type ThreadStatus } from './threadStatus'
import { useRoster } from './useRoster'
import { useConnection } from './useConnection'
import { useFeed } from './useFeed'
import { useThreads } from './useThreads'
import { useRequests } from './useRequests'
import { presenceOf } from './agentStateStyle'
import { useChannels } from '../channels/useChannels'
import { GENERAL_CHANNEL_NAME, LOGS_CHANNEL_NAME, TRIAGE_CHANNEL_NAME } from '../channels/channelRouting'
import { agentName } from './agents'
import UnreadDot from '../channels/UnreadDot.vue'
import type { UnreadState } from '../channels/unread'
import { useUnread } from '../channels/useUnread'
import type { ConversationScreen } from './timeline'
import type { Channel } from '../channels/channelsApi'
import ChannelDialog from '../channels/ChannelDialog.vue'
import ChannelMenu from '../channels/ChannelMenu.vue'
import type { Agent } from './events'
import { useAgentControl } from '../panel/useAgentControl'
import { SAME_FOLDER_TEXT, worktreeFolder, type AgentCopy } from './agentCopies'
import { useAgentCopies } from './useAgentCopies'
import { useBacklog } from '../backlog/useBacklog'
import { useProjects } from '../projects/useProjects'
import UiChip from '../../ui/UiChip.vue'
import UiHandDivider from '../../ui/UiHandDivider.vue'
import UiHandLine from '../../ui/UiHandLine.vue'
import UiIcon from '../../ui/UiIcon.vue'
import { HAND_DRAWN_RADIUS, HAND_MARK_CLASSES, INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import UiNotice from '../../ui/UiNotice.vue'

const NEW_CHANNEL_LABEL = 'Novo canal'
const TRIAGE_SCREEN: ConversationScreen = { kind: 'triage' }
const LOGS_SCREEN: ConversationScreen = { kind: 'logs' }
const OPEN_CARD_LABEL = 'Abrir task'
const OPEN_CARD_ICON_PATH = 'M3.5 3.5h9v9h-9z M6 8l1.5 1.5L10.5 6.5'
const OPEN_CARD_ICON_SIZE = 13
const THREAD_ICON_PATH = 'M8 10h8M8 14h5M5 4h14a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H9l-4 3v-3a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2Z'
const HOURGLASS_ICON_PATH = 'M6 2h12v4l-4 4 4 4v4H6v-4l4-4-4-4V2Z'
const GENERAL_ICON_PATH =
  'M2.3 4.2 Q7 2.8 11.4 4.1 Q12.3 7.2 11.2 9.8 Q7.8 10.6 5.2 10.2 L3.1 12.1 Q3.4 10.5 3.3 9.7 Q1.9 7.2 2.3 4.2 M12.2 6.5 Q14.1 7.2 13.9 9.7 Q13.8 11.5 12.6 12.1 L13.1 13.7 Q11.7 12.9 10.8 12.5 Q9.2 12.6 8.1 11.8'
const TRIAGE_ICON_PATH =
  'M2.4 3.4 Q8 2.8 13.6 3.5 Q11.4 6.6 9.5 8.8 Q9.3 11.2 9.4 13.3 Q8 13.9 6.6 13.1 Q6.7 10.9 6.5 8.8 Q4.5 6.5 2.4 3.4 M7.9 1.6 Q8.1 2.2 7.8 2.8'
const LOGS_ICON_PATH =
  'M2.6 3.3 Q8 2.7 13.4 3.4 Q13.9 8 13.3 12.7 Q8 13.3 2.7 12.6 Q2.1 8 2.6 3.3 M4.6 6.1 Q5.7 7 6.5 7.4 Q5.5 8.1 4.5 8.9 M7.6 8.9 Q9.4 8.7 11.2 9 M4.7 10.8 Q7.6 10.5 10.6 10.9'
const PANEL_CLASSES =
  'relative flex h-full min-h-0 w-full shrink-0 flex-col overflow-hidden p-1.5 max-md:max-h-[45vh] md:w-56 md:py-2 md:pl-1.5 md:pr-3 lg:w-64 xl:w-72'
const QUEUE_BADGE_CLASSES =
  'absolute right-1.5 top-1 grid min-w-4 place-items-center rounded-full bg-accent-soft px-1 text-tiny font-semibold text-accent'
const MENTION_LABEL = 'Mencionar'
const OPEN_CONVERSATION_PREFIX = 'Abrir conversa de '
const CARD_CLASSES = `flex w-full cursor-pointer items-center gap-2 rounded-md px-1.5 py-1 pr-7 text-left ${INK_FOCUS_CLASSES}`
const MENTION_CLASSES =
  `absolute right-1.5 bottom-1 grid size-5 cursor-pointer place-items-center rounded-full text-tiny font-semibold text-muted transition-[transform,color,background-color] duration-150 ease-out enabled:hover:scale-115 enabled:hover:bg-accent-soft enabled:hover:text-accent focus-visible:scale-115 focus-visible:bg-accent-soft focus-visible:text-accent disabled:opacity-40 motion-reduce:transition-none ${INK_FOCUS_CLASSES}`
const MENTION_GLYPH = '@'
const SELECTED_TEXT_CLASSES = 'text-ink-pen'
const UNSELECTED_CLASSES = 'hover:bg-surface-2'
const PORTRAIT_CIRCLE_CLASSES = 'pointer-events-none absolute -inset-1 size-[calc(100%+0.5rem)] text-ink-pen'
const CHANNEL_MARK_CLASSES = `${HAND_MARK_CLASSES} text-ink-pen! bottom-0.5 left-2 w-[calc(100%-2.75rem)]`
const CHANNEL_CLASSES = `relative flex w-full min-w-0 cursor-pointer items-center overflow-hidden rounded-md py-1 pl-2 pr-7 text-left text-label text-ink ${INK_FOCUS_CLASSES}`
const THREAD_CARD_BUTTON_CLASSES =
  `absolute right-1 top-1/2 grid size-5 -translate-y-1/2 cursor-pointer place-items-center rounded-md text-muted transition-colors duration-150 hover:bg-hover hover:text-ink motion-reduce:transition-none ${INK_FOCUS_CLASSES}`
const COPY_CLASSES = `flex w-full cursor-pointer items-start gap-1.5 rounded-md border border-line px-2 py-1 text-left transition-colors hover:bg-surface-2 ${INK_FOCUS_CLASSES}`
// COPY_CLASSES já fixa border-line; o `!` faz a tinta vencer.
const COPY_SELECTED_CLASSES = `border-[1.5px]! border-ink! bg-surface-2 ${HAND_DRAWN_RADIUS}`
const COPY_CONNECTOR_PATH = 'M5 3 C4.6 10 4.8 16 5.6 19.2 C8.5 20 13 19.7 17 19.4'
const COPY_THINKING_DOT_CLASSES = 'size-1 rounded-full bg-current animate-pulse motion-reduce:animate-none'
const THREADS_LABEL = 'Threads'
const THREAD_PORTRAIT_SIZE = 16
const THREAD_DOT: Partial<Record<ThreadStatus, { classes: string; label: string }>> = {
  working: { classes: 'bg-ok motion-safe:animate-pulse', label: 'trabalhando' },
  'needs-you': { classes: 'bg-danger motion-safe:animate-pulse', label: 'precisa de você' },
}
const COPIES_LIST_LABEL = 'Cópias'
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
const backlog = useBacklog()
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

const activeAgentId = computed<Agent | null>(() =>
  feed.screen.kind === 'agent' ? feed.screen.agent : null,
)

const queuedCounts = computed<Record<string, number>>(() => {
  const counts: Record<string, number> = {}
  for (const entry of requests.queue) {
    counts[entry.to] = (counts[entry.to] ?? 0) + 1
  }
  return counts
})

function queueCountLabel(count: number): string {
  return `${count} na fila`
}

function onJumpToAgent(agentId: Agent | null): void {
  if (!agentId) return
  feed.showScreen({ kind: 'agent', agent: agentId })
}

function isCopyShown(copyId: string): boolean {
  return feed.screen.kind === 'copy' && feed.screen.copyId === copyId
}

function openCopy(baseAgent: Agent, copy: AgentCopy): void {
  agentCopies.select(copy.id)
  feed.showScreen({
    kind: 'copy',
    copyId: copy.id,
    baseAgent,
    number: copy.number,
    itemId: copy.itemId,
    itemTitle: copy.itemTitle,
  })
}

function isCopyThinking(copyId: string): boolean {
  const state = feed.stateOf(copyId)
  return !control.isPaused(copyId) && (state === 'thinking' || state === 'sending')
}

const threads = useThreads()
const unread = useUnread()
const sidebarTab = ref<'canais' | 'agentes'>(
  feed.screen.kind === 'agent' || feed.screen.kind === 'copy' ? 'agentes' : 'canais',
)
watch(
  () => feed.screen.kind,
  (kind) => {
    if (kind === 'agent' || kind === 'copy') sidebarTab.value = 'agentes'
    else if (kind === 'channel' || kind === 'thread' || kind === 'triage') sidebarTab.value = 'canais'
  },
)
const threadCategories = computed(() => categorizeThreads(threads.threads))
const categoryOpenOverrides = ref<Partial<Record<ThreadCategoryKey, boolean>>>({})

function isCategoryOpen(category: { key: ThreadCategoryKey; defaultOpen: boolean; items: { itemId: string }[] }): boolean {
  const override = categoryOpenOverrides.value[category.key]
  if (override !== undefined) return override
  if (category.items.some((thread) => threadShown(thread.itemId))) return true
  return category.defaultOpen
}

function toggleCategory(key: ThreadCategoryKey, currentOpen: boolean): void {
  categoryOpenOverrides.value = { ...categoryOpenOverrides.value, [key]: !currentOpen }
}

// O item aberto já está à vista: a bolinha só aparece nos outros.
function indicatorOf(screen: ConversationScreen, shown: boolean): UnreadState {
  return shown ? 'read' : unread.stateOf(screen)
}

function triageShown(): boolean {
  return feed.screen.kind === 'triage'
}

function logsShown(): boolean {
  return feed.screen.kind === 'logs'
}

const errorLogCount = computed(() =>
  feed.feed.filter((entry) => entry.type === 'error' || entry.type === 'usage-limit-failed').length,
)

function threadAgentShown(itemId: string, agent: Agent): boolean {
  return feed.screen.kind === 'thread' && feed.screen.itemId === itemId && feed.screen.agent === agent
}

function channelShown(target: number | typeof GENERAL_CHANNEL_NAME): boolean {
  return feed.screen.kind === 'channel' && feed.screen.channel === target
}

function threadShown(itemId: string): boolean {
  return feed.screen.kind === 'thread' && feed.screen.itemId === itemId
}

function openThreadCard(itemId: string): void {
  backlog.open(itemId)
}

function shortThreadId(thread: { itemId: string; publicId?: string }): string | null {
  const raw = (thread.publicId ?? thread.itemId).trim()
  return raw.length > 0 && raw.length <= 12 ? raw : null
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
    <UiHandDivider class="absolute inset-y-0 right-0 h-full max-md:hidden" />
    <div :class="['flex min-h-0 flex-1 flex-col', stripCollapsed && 'max-md:hidden']">
      <div
        role="tablist"
        aria-label="Alternar entre canais e agentes"
        class="mb-2 grid shrink-0 grid-cols-2 gap-1 rounded-md border border-line bg-surface-2 p-0.5"
      >
        <button
          type="button"
          role="tab"
          :aria-selected="sidebarTab === 'canais'"
          class="cursor-pointer rounded-sm px-2 py-1 text-tiny font-semibold transition-colors"
          :class="sidebarTab === 'canais' ? 'bg-surface text-ink shadow-seg' : 'text-muted hover:text-ink'"
          data-testid="sidebar-tab-canais"
          @click="sidebarTab = 'canais'"
        >
          Canais / Threads
        </button>
        <button
          type="button"
          role="tab"
          :aria-selected="sidebarTab === 'agentes'"
          class="cursor-pointer rounded-sm px-2 py-1 text-tiny font-semibold transition-colors"
          :class="sidebarTab === 'agentes' ? 'bg-surface text-ink shadow-seg' : 'text-muted hover:text-ink'"
          data-testid="sidebar-tab-agentes"
          @click="sidebarTab = 'agentes'"
        >
          Agentes ({{ roster.agents.length }})
        </button>
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto pr-1">
      <template v-if="sidebarTab === 'canais'">
      <div class="mb-1 flex flex-wrap items-center justify-between gap-1.5">
        <h2 class="text-small font-semibold text-muted">Canais</h2>
        <UiChip kind="add" :label="`+ ${NEW_CHANNEL_LABEL}`" @press="openChannelDialog()" />
      </div>
      <ul class="mb-1.5 flex flex-col gap-0.5">
        <li>
          <button
            type="button"
            :class="[CHANNEL_CLASSES, 'gap-1.5', channelShown(GENERAL_CHANNEL_NAME) ? SELECTED_TEXT_CLASSES : UNSELECTED_CLASSES]"
            :aria-label="`Abrir canal #${GENERAL_CHANNEL_NAME}`"
            :aria-pressed="channelShown(GENERAL_CHANNEL_NAME)"
            @click="feed.showScreen({ kind: 'channel', channel: GENERAL_CHANNEL_NAME })"
          >
            <svg viewBox="0 0 16 16" class="size-3.5 shrink-0" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path :d="GENERAL_ICON_PATH" />
            </svg>
            {{ GENERAL_CHANNEL_NAME }}
            <UnreadDot
              :state="indicatorOf({ kind: 'channel', channel: GENERAL_CHANNEL_NAME }, channelShown(GENERAL_CHANNEL_NAME))"
              class="ml-auto"
            />
            <UiHandLine v-if="channelShown(GENERAL_CHANNEL_NAME)" draw :class="CHANNEL_MARK_CLASSES" />
          </button>
        </li>
        <li>
          <button
            type="button"
            :class="[CHANNEL_CLASSES, 'gap-1.5', triageShown() ? SELECTED_TEXT_CLASSES : UNSELECTED_CLASSES]"
            :aria-label="`Abrir canal ${TRIAGE_CHANNEL_NAME}`"
            :aria-pressed="triageShown()"
            @click="feed.showScreen(TRIAGE_SCREEN)"
          >
            <svg viewBox="0 0 16 16" class="size-3.5 shrink-0" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path :d="TRIAGE_ICON_PATH" />
            </svg>
            {{ TRIAGE_CHANNEL_NAME }}
            <UnreadDot :state="indicatorOf(TRIAGE_SCREEN, triageShown())" class="ml-auto" />
            <UiHandLine v-if="triageShown()" draw :class="CHANNEL_MARK_CLASSES" />
          </button>
        </li>
        <li>
          <button
            type="button"
            :class="[CHANNEL_CLASSES, 'gap-1.5', logsShown() ? SELECTED_TEXT_CLASSES : UNSELECTED_CLASSES]"
            aria-label="Abrir canal #logs"
            :aria-pressed="logsShown()"
            data-testid="channel-logs"
            @click="feed.showScreen(LOGS_SCREEN)"
          >
            <svg viewBox="0 0 16 16" class="size-3.5 shrink-0" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path :d="LOGS_ICON_PATH" />
            </svg>
            <span class="truncate">{{ LOGS_CHANNEL_NAME }}</span>
            <span v-if="errorLogCount > 0" class="ml-auto rounded-full bg-danger/15 px-1.5 text-tiny font-semibold text-danger tabular-nums">
              {{ errorLogCount }}
            </span>
            <UiHandLine v-if="logsShown()" draw :class="CHANNEL_MARK_CLASSES" />
          </button>
        </li>
        <li v-for="channel in channels.channels" :key="channel.id" class="group relative">
          <button
            type="button"
            :class="[CHANNEL_CLASSES, channelShown(channel.id) ? SELECTED_TEXT_CLASSES : UNSELECTED_CLASSES]"
            :aria-label="`Abrir canal #${channel.name}`"
            :aria-pressed="channelShown(channel.id)"
            @click="feed.showScreen({ kind: 'channel', channel: channel.id })"
          >
            #{{ channel.name }}
            <UnreadDot :state="indicatorOf({ kind: 'channel', channel: channel.id }, channelShown(channel.id))" class="ml-auto" />
            <UiHandLine v-if="channelShown(channel.id)" draw :class="CHANNEL_MARK_CLASSES" />
          </button>
          <ChannelMenu :name="channel.name" @edit="openChannelDialog(channel)" @remove="removeChannel(channel)" />
        </li>
      </ul>
      <template v-if="threadCategories.length > 0">
        <h2 class="mb-1 mt-0.5 text-small font-semibold text-muted">{{ THREADS_LABEL }}</h2>
        <div v-for="category in threadCategories" :key="category.key" class="mb-1.5">
          <button
            type="button"
            class="mb-0.5 flex w-fit cursor-pointer items-center gap-1 text-small font-semibold text-muted hover:text-ink"
            :class="INK_FOCUS_CLASSES"
            :aria-expanded="isCategoryOpen(category)"
            :aria-controls="`threads-${category.key}`"
            :data-testid="`threads-toggle-${category.key}`"
            @click="toggleCategory(category.key, isCategoryOpen(category))"
          >
            <span aria-hidden="true">{{ isCategoryOpen(category) ? '▾' : '▸' }}</span>
            <span>{{ category.label }} ({{ category.items.length }})</span>
          </button>
          <ul v-show="isCategoryOpen(category)" :id="`threads-${category.key}`" class="flex flex-col gap-0.5">
            <li v-for="thread in category.items" :key="`${category.key}-${thread.itemId}`" class="group relative">
              <button
                type="button"
                :class="[
                  CHANNEL_CLASSES,
                  'flex items-center gap-1.5',
                  category.key === 'done' && 'text-muted',
                  threadShown(thread.itemId) ? SELECTED_TEXT_CLASSES : UNSELECTED_CLASSES,
                ]"
                :aria-label="`Abrir thread ${thread.title}`"
                :aria-pressed="threadShown(thread.itemId)"
                @click="feed.showScreen({ kind: 'thread', itemId: thread.itemId })"
              >
                <UiIcon
                  v-if="thread.isTriaging || !thread.assignee || category.key === 'done'"
                  :path="thread.isTriaging ? HOURGLASS_ICON_PATH : THREAD_ICON_PATH"
                  class="h-3.5 w-3.5 shrink-0"
                />
                <AgentPortrait v-else :who="thread.assignee" :size="THREAD_PORTRAIT_SIZE" class="shrink-0" />
                <span v-if="shortThreadId(thread)" class="max-w-16 shrink-0 truncate font-mono text-tiny font-semibold text-muted">{{ shortThreadId(thread) }}</span>
                <span class="min-w-0 flex-1 truncate">{{ thread.title }}</span>
                <span
                  v-if="category.key !== 'done' && thread.exchangeCount > 0"
                  class="shrink-0 rounded-sm bg-surface-2 px-1 text-[10px] font-medium text-muted tabular-nums"
                  :title="`${thread.exchangeCount} mensagens entre agentes`"
                >
                  ↔{{ thread.exchangeCount }}
                </span>
                <span
                  v-if="category.key !== 'done' && THREAD_DOT[thread.status]"
                  class="ml-auto size-1.5 shrink-0 rounded-full"
                  :class="THREAD_DOT[thread.status]?.classes"
                  :title="THREAD_DOT[thread.status]?.label"
                  role="img"
                  :aria-label="THREAD_DOT[thread.status]?.label"
                />
                <UnreadDot
                  v-if="category.key !== 'done'"
                  :state="indicatorOf({ kind: 'thread', itemId: thread.itemId }, threadShown(thread.itemId))"
                  :class="THREAD_DOT[thread.status] ? '' : 'ml-auto'"
                />
                <UiHandLine v-if="threadShown(thread.itemId)" draw :class="CHANNEL_MARK_CLASSES" />
              </button>
              <button
                type="button"
                :class="THREAD_CARD_BUTTON_CLASSES"
                :title="`${OPEN_CARD_LABEL}: ${thread.title}`"
                :aria-label="`${OPEN_CARD_LABEL}: ${thread.title}`"
                @click="openThreadCard(thread.itemId)"
              >
                <UiIcon :path="OPEN_CARD_ICON_PATH" :size="OPEN_CARD_ICON_SIZE" />
              </button>
              <ul
                v-if="category.key !== 'done' && (thread.participants.length > 1 || threadShown(thread.itemId)) && thread.participants.length > 0"
                class="mt-0.5 ml-4 flex flex-col gap-0.5 border-l border-line pl-2"
              >
                <li v-for="participant in thread.participants" :key="`${thread.itemId}-${participant}`">
                  <button
                    type="button"
                    class="flex w-full min-w-0 cursor-pointer items-center gap-1.5 rounded-sm px-1.5 py-0.5 text-left text-tiny transition-colors"
                    :class="threadAgentShown(thread.itemId, participant) ? 'bg-surface-2 font-semibold text-ink' : 'text-muted hover:bg-surface-2 hover:text-ink'"
                    @click="feed.showScreen({ kind: 'thread', itemId: thread.itemId, agent: participant })"
                  >
                    <AgentPortrait :who="participant" :size="14" class="shrink-0" />
                    <span class="min-w-0 flex-1 truncate">{{ agentName(participant) }}</span>
                    <span
                      v-if="feed.stateOf(participant) === 'thinking' || feed.stateOf(participant) === 'sending'"
                      class="size-1.5 shrink-0 rounded-full bg-ok animate-pulse"
                    />
                  </button>
                </li>
              </ul>
            </li>
          </ul>
        </div>
      </template>
      <UiNotice v-if="channelError" class="mb-2" :text="channelError" @close="channelError = null" />
      </template>
      <template v-else>
      <div class="mb-1 flex items-center justify-between gap-1.5">
        <h2 class="text-small font-semibold text-muted">Agentes</h2>
        <AgentPicker
          v-if="roster.agents.length > 0"
          :model-value="activeAgentId"
          :counts="queuedCounts"
          :count-label="queueCountLabel"
          busy-first
          floating="fixed"
          trigger-variant="ghost"
          trigger-class="h-6 px-1.5 text-tiny"
          label="Ir rapidamente para um agente"
          title="Buscar e alternar agente"
          test-id="agent-list-picker"
          @select="onJumpToAgent"
        >
          <template #trigger>
            <svg
              class="size-3 shrink-0"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <circle cx="11" cy="11" r="8" />
              <line x1="21" y1="21" x2="16.65" y2="16.65" />
            </svg>
            <span>Buscar</span>
          </template>
        </AgentPicker>
      </div>
      <ul class="flex flex-col gap-0.5">
        <li v-for="agent in roster.agents" :key="agent.id">
          <div class="relative">
            <button
              type="button"
              :class="[CARD_CLASSES, isShown(agent.id) ? SELECTED_TEXT_CLASSES : UNSELECTED_CLASSES]"
              :aria-label="OPEN_CONVERSATION_PREFIX + agent.name"
              :aria-pressed="isShown(agent.id)"
              @click="feed.showScreen({ kind: 'agent', agent: agent.id })"
            >
              <span class="relative flex shrink-0" aria-hidden="true">
                <AgentPortrait :who="agent.id" :size="PORTRAIT_SIZES.stack" />
                <UiHandLine v-if="isShown(agent.id)" variant="circle" draw :class="PORTRAIT_CIRCLE_CLASSES" />
              </span>
              <span class="flex min-w-0 flex-1 flex-col">
                <span class="flex min-w-0 items-center gap-1.5 text-label font-semibold">
                  <span class="truncate">{{ agent.name }}</span>
                  <AgentProjectIcons :agent="agent.id" />
                  <span v-if="agentCopies.copiesOf(agent.id).length" class="ml-auto shrink-0 whitespace-nowrap text-tiny font-normal text-muted">
                    {{ copiesLabel(agentCopies.copiesOf(agent.id).length) }}
                  </span>
                </span>
                <span class="flex min-w-0 items-center gap-1 text-tiny text-ink">
                  <span class="size-1.5 shrink-0 rounded-full" :class="presence(agent.id).dot" />
                  <span class="shrink-0">{{ presence(agent.id).label }}</span>
                  <span class="truncate text-muted">· {{ agent.role }}</span>
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
          </div>
          <ul
            v-if="agentCopies.copiesOf(agent.id).length"
            :aria-label="`${COPIES_LIST_LABEL} de ${agent.name}`"
            class="mt-0.5 ml-2 flex flex-col gap-0.5"
          >
            <li v-for="copy in agentCopies.copiesOf(agent.id)" :key="copy.id">
              <button
                type="button"
                :class="[COPY_CLASSES, isCopyShown(copy.id) && COPY_SELECTED_CLASSES]"
                :aria-pressed="isCopyShown(copy.id)"
                @click="openCopy(agent.id, copy)"
              >
                <svg
                  viewBox="0 0 20 24"
                  class="mt-0.5 h-5 w-4 shrink-0 text-muted"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.5"
                  stroke-linecap="round"
                  aria-hidden="true"
                >
                  <path :d="COPY_CONNECTOR_PATH" />
                </svg>
                <span class="flex min-w-0 flex-1 flex-col">
                  <span class="flex items-center justify-between gap-2">
                    <span class="text-label font-semibold text-ink">{{ agent.name }} {{ copy.number }}</span>
                    <span
                      v-if="isCopyThinking(copy.id)"
                      class="inline-flex shrink-0 items-center gap-0.5 text-ink-pen"
                      :title="presence(copy.id).label"
                      :aria-label="presence(copy.id).label"
                    >
                      <span :class="COPY_THINKING_DOT_CLASSES" />
                      <span :class="[COPY_THINKING_DOT_CLASSES, '[animation-delay:150ms]']" />
                      <span :class="[COPY_THINKING_DOT_CLASSES, '[animation-delay:300ms]']" />
                    </span>
                    <span
                      v-else
                      class="size-1.5 shrink-0 rounded-full"
                      :class="presence(copy.id).dot"
                      :title="presence(copy.id).label"
                    />
                  </span>
                  <span v-if="copy.itemId" class="truncate text-tiny text-muted">
                    {{ copy.itemId }}<template v-if="copy.itemTitle"> · {{ copy.itemTitle }}</template>
                  </span>
                  <span class="truncate font-mono text-tiny text-muted">
                    {{ copy.worktreePath ? worktreeFolder(copy.worktreePath) : SAME_FOLDER_TEXT }}
                  </span>
                </span>
              </button>
            </li>
          </ul>
        </li>
      </ul>
      </template>
      </div>
    </div>
  </aside>
  <ChannelDialog v-if="creating" :key="editing?.id" :channel="editing ?? undefined" @close="closeChannelDialog" />
</template>
