<script setup lang="ts">
import { computed, ref } from 'vue'
import UnreadDot from '../channels/UnreadDot.vue'
import type { UnreadState } from '../channels/unread'
import { useUnread } from '../channels/useUnread'
import { useBacklog } from '../backlog/useBacklog'
import { useRunItem } from '../backlog/useRunItem'
import UiHandLine from '../../ui/UiHandLine.vue'
import UiIcon from '../../ui/UiIcon.vue'
import { HAND_MARK_CLASSES, INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import AgentPortrait from './AgentPortrait.vue'
import type { ThreadCategoryKey, ThreadStatus } from './threadStatus'
import type { ConversationScreen } from './timeline'
import { useFeed } from './useFeed'
import type { TaskThread } from './useThreads'

defineOptions({ name: 'ThreadTreeNode' })

const props = defineProps<{
  thread: TaskThread
  categoryKey: ThreadCategoryKey
}>()

const OPEN_CARD_LABEL = 'Abrir task'
const OPEN_CARD_ICON_PATH = 'M3.5 3.5h9v9h-9z M6 8l1.5 1.5L10.5 6.5'
const OPEN_CARD_ICON_SIZE = 13
const START_THREAD_LABEL = 'Retomar / verificar'
const START_THREAD_ICON_PATH = 'M5 3.5l7.5 4.5L5 12.5z'
const THREAD_PORTRAIT_SIZE = 16
const SELECTED_TEXT_CLASSES = 'text-ink-pen'
const UNSELECTED_CLASSES = 'hover:bg-surface-2'
const CHANNEL_MARK_CLASSES = `${HAND_MARK_CLASSES} text-ink-pen! bottom-0.5 left-2 w-[calc(100%-2.75rem)]`
const CHANNEL_CLASSES = `relative flex w-full min-w-0 cursor-pointer items-center overflow-hidden rounded-md py-1 pl-2 pr-7 text-left text-label text-ink ${INK_FOCUS_CLASSES}`
const THREAD_CARD_BUTTON_CLASSES =
  `absolute right-1 top-1/2 grid size-5 -translate-y-1/2 cursor-pointer place-items-center rounded-md text-muted transition-colors duration-150 hover:bg-hover hover:text-ink motion-reduce:transition-none ${INK_FOCUS_CLASSES}`
const THREAD_DOT: Partial<Record<ThreadStatus, { classes: string; label: string }>> = {
  working: { classes: 'bg-ok motion-safe:animate-pulse', label: 'trabalhando' },
  'needs-you': { classes: 'bg-danger motion-safe:animate-pulse', label: 'precisa de você' },
}

const feed = useFeed()
const backlog = useBacklog()
const runItem = useRunItem()
const unread = useUnread()

const subthreadsOverride = ref<boolean | null>(null)

function hasSelectedDescendant(node: TaskThread): boolean {
  if (feed.screen.kind === 'thread' && feed.screen.itemId === node.itemId) return true
  return node.subthreads.some(hasSelectedDescendant)
}

const subthreadsOpen = computed<boolean>(() => {
  if (subthreadsOverride.value !== null) return subthreadsOverride.value
  if (props.thread.subthreads.some(hasSelectedDescendant)) return true
  return props.thread.subthreads.some((sub) => sub.status === 'working' || sub.status === 'needs-you')
})

function toggleSubthreads(): void {
  subthreadsOverride.value = !subthreadsOpen.value
}

function threadShown(itemId: string): boolean {
  return feed.screen.kind === 'thread' && feed.screen.itemId === itemId
}

function indicatorOf(screen: ConversationScreen, shown: boolean): UnreadState {
  return shown ? 'read' : unread.stateOf(screen)
}

function shortThreadId(thread: { itemId: string; publicId?: string }): string | null {
  const raw = (thread.publicId ?? thread.itemId).trim()
  return raw.length > 0 && raw.length <= 12 ? raw : null
}
const isThreadActive = computed(
  () => runItem.isRunning(props.thread.itemId) || props.thread.status === 'working',
)
</script>

<template>
  <li class="flex flex-col">
    <div class="group relative">
      <button
        type="button"
        :class="[
          CHANNEL_CLASSES,
          'flex items-center gap-1.5',
          runItem.canRun(thread.itemId) && 'pr-12',
          categoryKey === 'done' && 'text-muted',
          threadShown(thread.itemId) ? SELECTED_TEXT_CLASSES : UNSELECTED_CLASSES,
        ]"
        :aria-label="`Abrir thread ${thread.title}`"
        :aria-pressed="threadShown(thread.itemId)"
        @click="feed.showScreen({ kind: 'thread', itemId: thread.itemId })"
      >
        <span
          v-if="thread.subthreads.length > 0"
          role="button"
          tabindex="0"
          class="grid size-4 shrink-0 place-items-center rounded-sm text-[10px] text-muted hover:bg-hover hover:text-ink"
          :title="subthreadsOpen ? 'Recolher subthreads' : 'Expandir subthreads'"
          :aria-expanded="subthreadsOpen"
          @click.stop="toggleSubthreads"
          @keydown.enter.stop.prevent="toggleSubthreads"
          @keydown.space.stop.prevent="toggleSubthreads"
        >
          {{ subthreadsOpen ? '▾' : '▸' }}
        </span>
        <AgentPortrait
          v-if="thread.assignee && categoryKey !== 'done'"
          :who="thread.assignee"
          :size="THREAD_PORTRAIT_SIZE"
          class="shrink-0"
        />
        <span v-if="shortThreadId(thread)" class="max-w-16 shrink-0 truncate font-mono text-tiny font-semibold text-muted">{{ shortThreadId(thread) }}</span>
        <span class="min-w-0 flex-1 truncate">{{ thread.title }}</span>
        <span
          v-if="thread.subthreads.length > 0"
          class="shrink-0 rounded-sm bg-surface-2 px-1 text-[10px] font-medium text-muted tabular-nums"
          :title="`${thread.subthreads.length} subthread(s)`"
        >
          {{ thread.subthreads.length }}
        </span>
        <span
          v-if="categoryKey !== 'done' && thread.exchangeCount > 0"
          class="shrink-0 rounded-sm bg-surface-2 px-1 text-[10px] font-medium text-muted tabular-nums"
          :title="`${thread.exchangeCount} mensagens entre agentes`"
        >
          ↔{{ thread.exchangeCount }}
        </span>
        <span
          v-if="categoryKey !== 'done' && THREAD_DOT[thread.status]"
          class="ml-auto size-1.5 shrink-0 rounded-full"
          :class="THREAD_DOT[thread.status]?.classes"
          :title="THREAD_DOT[thread.status]?.label"
          role="img"
          :aria-label="THREAD_DOT[thread.status]?.label"
        />
        <UnreadDot
          v-if="categoryKey !== 'done'"
          :state="indicatorOf({ kind: 'thread', itemId: thread.itemId }, threadShown(thread.itemId))"
          :class="THREAD_DOT[thread.status] ? '' : 'ml-auto'"
        />
        <UiHandLine v-if="threadShown(thread.itemId)" draw :class="CHANNEL_MARK_CLASSES" />
      </button>
      <button
        v-if="runItem.canRun(thread.itemId) && !isThreadActive"
        type="button"
        :class="[THREAD_CARD_BUTTON_CLASSES, 'right-6 text-ok hover:text-ok']"
        :title="`${START_THREAD_LABEL}: ${thread.title}`"
        :aria-label="`${START_THREAD_LABEL}: ${thread.title}`"
        @click.stop="void runItem.run(thread.itemId)"
      >
        <UiIcon :path="START_THREAD_ICON_PATH" :size="OPEN_CARD_ICON_SIZE" />
      </button>
      <button
        type="button"
        :class="THREAD_CARD_BUTTON_CLASSES"
        :title="`${OPEN_CARD_LABEL}: ${thread.title}`"
        :aria-label="`${OPEN_CARD_LABEL}: ${thread.title}`"
        @click="backlog.open(thread.itemId)"
      >
        <UiIcon :path="OPEN_CARD_ICON_PATH" :size="OPEN_CARD_ICON_SIZE" />
      </button>
    </div>

    <ul
      v-if="thread.subthreads.length > 0 && subthreadsOpen"
      class="mt-0.5 ml-4 flex flex-col gap-0.5 border-l border-line pl-2"
      data-testid="subthreads-tree"
    >
      <ThreadTreeNode
        v-for="sub in thread.subthreads"
        :key="`${categoryKey}-${sub.itemId}`"
        :thread="sub"
        :category-key="categoryKey"
      />
    </ul>
  </li>
</template>
