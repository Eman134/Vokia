<script setup lang="ts">
import { computed } from 'vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import UiBadge from '../../ui/UiBadge.vue'
import UiSketchBox from '../../ui/UiSketchBox.vue'
import UiMarker from '../../ui/UiMarker.vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import { useRoster } from '../team/useRoster'
import { formatItemUsage } from '../team/itemUsage'
import { useItemUsage } from '../team/useItemUsage'
import UserAvatar from '../profile/UserAvatar.vue'
import {
  BADGE_DOT,
  AUTO_CLOSED_LABEL,
  CLOSED_BY_AUTOMATIC,
  COLUMN_CHIP,
  STATE_BADGE,
  STATE_CHIP,
  TRIAGE_STEP_COUNT,
} from './backlogLabels'
import type { ParsedProposal } from './planProposal'
import {
  isUnassigned,
  UNASSIGNED_TEXT,
  type TaskCardAction,
  type BacklogColumn,
  type BacklogItem,
  type BacklogState,
} from './backlogTypes'
import { cardActionFor } from './cardAction'
import { openQuestionsCount } from './backlogFilters'
import BacklogPlanActions from './BacklogPlanActions.vue'
import { extractAttachments } from '../attachments/attachmentBlock'
import AttachmentPreviewList from '../attachments/AttachmentPreviewList.vue'

export type CardDensity = 'full' | 'compact'

const props = withDefaults(
  defineProps<{
    item: BacklogItem
    column: BacklogColumn
    state: BacklogState
    planLine?: string
    proposal?: ParsedProposal
    sprintName?: string
    density?: CardDensity
    selectable?: boolean
    selected?: boolean
    queued?: boolean
    dragging?: boolean
    floating?: boolean
  }>(),
  {
    density: 'full',
    selectable: false,
    selected: false,
    queued: false,
    dragging: false,
    floating: false,
  },
)
const emit = defineEmits<{
  open: []
  action: [action: TaskCardAction]
  toggleSelect: [shiftKey: boolean]
}>()

const DONE_SEGMENTS = 1
const DONE_COLUMN: BacklogColumn = 'Done'
const AVATAR_SIZE = 22
const COMPACT_AVATAR_SIZE = 18
const MAX_VISIBLE_TAGS = 3
const FULL_CARD_CLASSES =
  'group relative flex min-w-0 cursor-pointer flex-col gap-1 rounded-sm border border-line bg-surface px-2 py-2 lg:gap-1.5 lg:px-3 lg:py-2.5'
const COMPACT_CARD_CLASSES =
  'group relative flex min-w-0 cursor-pointer select-none flex-col gap-1 rounded-sm border border-line bg-surface px-2 py-1.5'
const SELECTED_CARD_CLASSES = 'border-accent bg-accent-soft'
const DRAGGING_GHOST_CLASSES = 'opacity-35 border-dashed border-graphite bg-surface-2 pointer-events-none select-none'
const FLOATING_CARD_CLASSES = 'rotate-3 scale-[1.02] shadow-paper ring-1 ring-ink/15 pointer-events-none select-none'
const CHECKBOX_BASE_CLASSES = `size-4 shrink-0 cursor-pointer items-center justify-center rounded-sm border transition-colors ${INK_FOCUS_CLASSES}`
const CHECKBOX_ON_CLASSES = 'inline-flex border-accent bg-accent text-accent-ink'
const CHECKBOX_OFF_CLASSES = 'hidden border-line bg-surface text-transparent hover:border-ink group-hover:inline-flex group-focus-within:inline-flex'
const ID_CLASSES = 'shrink-0 rounded-sm bg-surface-2 px-1.5 py-0.5 font-mono text-tiny font-bold tracking-wide text-ink'
const TAG_CLASSES = 'max-w-[110px] truncate rounded-sm border border-line bg-surface-2 px-1.5 py-0.5 text-tiny text-muted'
const PAPERCLIP_PATH = 'M11.5 4.5 6.3 9.7a2 2 0 1 1-2.8-2.8l5.6-5.6a3.2 3.2 0 0 1 4.5 4.5l-5.7 5.7a4.4 4.4 0 0 1-6.2-6.2l5.2-5.2'
const CHAT_BUBBLE_PATH = 'M2.5 3.5h11v7.5H7l-3 2.5V11H2.5z'
const CHECK_PATH = 'M3.5 8.5 6.5 11.5 12.5 5'
const QUEUED_TEXT = 'Na fila de triagem…'
const GENERATING_TEXT = 'Escrevendo o plano…'

const roster = useRoster()
const itemUsage = useItemUsage(() => props.item.id)
const usageText = computed(() => formatItemUsage(itemUsage.value))
const displayId = computed(() => props.item.publicId ?? props.item.id)
const visibleTags = computed(() => props.item.tags.slice(0, MAX_VISIBLE_TAGS))
const extraTagsCount = computed(() => Math.max(0, props.item.tags.length - MAX_VISIBLE_TAGS))
const descriptionContent = computed(() => extractAttachments(props.item.description ?? ''))
const compactDescription = computed(() => descriptionContent.value.text.trim())
const attachmentCount = computed(() => descriptionContent.value.attachments.length)
const interactionCount = computed(() => props.item.interactionCount ?? 0)
const openQuestions = computed(() => openQuestionsCount(props.item))
const isCompact = computed(() => props.density === 'compact')

const chip = computed(
  () => COLUMN_CHIP[props.column] ?? { label: STATE_CHIP[props.state], status: STATE_BADGE[props.state] },
)
const isFeature = computed(() => props.item.type === 'feature')
const isAutoClosed = computed(() => props.column === DONE_COLUMN && props.item.closedBy === CLOSED_BY_AUTOMATIC)
const ownerName = computed(() => (isUnassigned(props.item) ? UNASSIGNED_TEXT : (roster.profileOf(props.item.who)?.name ?? props.item.who)))

const action = computed(() => cardActionFor(props.item, props.column, props.state))

const progress = computed(() => {
  const { children, triageStep } = props.item
  if (props.column === DONE_COLUMN) {
    const total = Math.max(children?.total ?? 0, DONE_SEGMENTS)
    return { total, filled: total, fillClass: 'bg-ok', showText: children !== undefined && children.total > 0, statusText: undefined }
  }
  if (children) {
    return { total: children.total, filled: Math.min(children.done, children.total), fillClass: 'bg-ok', showText: true, statusText: undefined }
  }
  if (props.state === 'triagem') {
    return { total: TRIAGE_STEP_COUNT, filled: triageStep ?? 0, fillClass: 'bg-accent', showText: false, statusText: props.planLine }
  }
  if (props.state === 'gerando') {
    return { total: TRIAGE_STEP_COUNT, filled: 1, fillClass: 'bg-accent', showText: false, statusText: props.planLine ?? GENERATING_TEXT }
  }
  if (props.queued) {
    return { total: TRIAGE_STEP_COUNT, filled: 0, fillClass: 'bg-accent', showText: false, statusText: QUEUED_TEXT }
  }
  return undefined
})

const percent = computed(() => {
  if (!progress.value || progress.value.total === 0) return 0
  return Math.round((progress.value.filled / progress.value.total) * 100)
})

function onCardClick(event: MouseEvent): void {
  if (props.selectable && event.shiftKey) {
    emit('toggleSelect', true)
    return
  }
  emit('open')
}

</script>

<template>
  <article
    tabindex="0"
    :data-card-id="item.id"
    :class="[
      isCompact ? COMPACT_CARD_CLASSES : FULL_CARD_CLASSES,
      !item.error && !floating && 'cursor-grab active:cursor-grabbing',
      selected && SELECTED_CARD_CLASSES,
      dragging && DRAGGING_GHOST_CLASSES,
      floating && FLOATING_CARD_CLASSES,
      INK_FOCUS_CLASSES,
    ]"
    @click="onCardClick"
    @keydown.enter.self="emit('open')"
  >
    <UiSketchBox v-if="item.error" tone="danger" class="pointer-events-none absolute inset-0" />
    <header class="flex flex-wrap items-center gap-1.5">
      <span class="size-2 shrink-0 rounded-full" :class="BADGE_DOT[chip.status]" :title="chip.label" aria-hidden="true" />
      <span class="sr-only">{{ chip.label }}:</span>
      <span :class="ID_CLASSES" data-testid="card-id">{{ displayId }}</span>
      <UiMarker v-if="isFeature" tone="blue" class="text-tiny font-semibold">feature</UiMarker>
      <span
        v-if="!isCompact && sprintName"
        class="max-w-[120px] truncate rounded-sm border border-line bg-surface-2 px-1.5 py-0.5 text-tiny text-muted"
        :title="`Sprint: ${sprintName}`"
        data-testid="card-sprint-badge"
      >
        {{ sprintName }}
      </span>
      <UiBadge v-if="isAutoClosed" status="trabalhando" :label="AUTO_CLOSED_LABEL" />
      <div v-if="isCompact || selectable" class="ml-auto flex shrink-0 items-center gap-1.5">
        <span
          v-if="isCompact && interactionCount > 0"
          class="inline-flex shrink-0 items-center gap-0.5 text-tiny tabular-nums"
          :class="openQuestions > 0 ? 'font-semibold text-warn' : 'text-muted'"
          :title="openQuestions > 0 ? `${interactionCount} interações (${openQuestions} pergunta aberta)` : `${interactionCount} interações`"
          data-testid="card-compact-interactions"
        >
          <svg viewBox="0 0 16 16" class="size-3 shrink-0" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path :d="CHAT_BUBBLE_PATH" />
          </svg>
          {{ interactionCount }}
        </span>
        <span
          v-if="isCompact && attachmentCount > 0"
          class="inline-flex shrink-0 items-center gap-0.5 text-tiny text-muted tabular-nums"
          :title="`${attachmentCount} ${attachmentCount === 1 ? 'anexo' : 'anexos'}`"
          data-testid="card-attachment-count"
        >
          <svg viewBox="0 0 16 16" class="size-3 shrink-0" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path :d="PAPERCLIP_PATH" />
          </svg>
          {{ attachmentCount }}
        </span>
        <button
          v-if="selectable"
          type="button"
          role="checkbox"
          :aria-checked="selected"
          :aria-label="`Selecionar ${item.title}`"
          :class="[CHECKBOX_BASE_CLASSES, selected ? CHECKBOX_ON_CLASSES : CHECKBOX_OFF_CLASSES]"
          data-testid="card-select"
          @click.stop="emit('toggleSelect', $event.shiftKey)"
        >
          <svg viewBox="0 0 16 16" class="size-3" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path :d="CHECK_PATH" />
          </svg>
        </button>
        <span v-if="isCompact" class="shrink-0" :title="ownerName">
          <UserAvatar v-if="isUnassigned(item)" :size="COMPACT_AVATAR_SIZE" />
          <AgentPortrait v-else :who="item.who" :size="COMPACT_AVATAR_SIZE" />
        </span>
      </div>
    </header>

    <p v-if="item.parent && !isCompact" class="hidden truncate text-meta text-muted lg:block">↳ {{ item.parent }}</p>
    <h3 class="min-w-0 line-clamp-2 text-caption lg:text-body leading-[1.3] font-bold text-ink" :title="item.title">
      {{ item.title }}
    </h3>
    <div
      v-if="isCompact && compactDescription"
      class="hidden group-hover:block group-focus-within:block"
      data-testid="card-compact-description"
    >
      <p class="line-clamp-3 text-meta leading-[1.35] text-muted">{{ compactDescription }}</p>
    </div>
    <div v-if="visibleTags.length > 0 && !isCompact" class="hidden flex-wrap items-center gap-1 lg:flex" data-testid="card-tags">
      <span v-for="tag in visibleTags" :key="tag" :class="TAG_CLASSES" :title="tag">{{ tag }}</span>
      <span v-if="extraTagsCount > 0" class="text-tiny text-muted">+{{ extraTagsCount }}</span>
    </div>
    <p v-if="usageText && !isCompact" class="hidden text-meta text-muted xl:block" data-testid="card-usage">{{ usageText }}</p>
    <AttachmentPreviewList
      v-if="descriptionContent.attachments.length > 0 && !isCompact"
      :attachments="descriptionContent.attachments"
      compact
      class="hidden lg:flex"
      data-testid="card-attachments"
    />
    <p v-if="item.error" role="alert" class="text-caption font-semibold text-danger">
      Erro ao ler este item: {{ item.error.reason }}
    </p>

    <div
      v-if="isCompact && action"
      class="hidden items-center gap-1.5 pt-0.5 group-hover:flex group-focus-within:flex"
      data-testid="card-compact-actions"
    >
      <BacklogPlanActions :action="action" @action="emit('action', $event)" />
    </div>

    <div v-if="!isCompact" class="mt-auto flex items-center gap-1.5 pt-0.5 lg:gap-2">
      <BacklogPlanActions v-if="action" :action="action" class="hidden lg:flex" @action="emit('action', $event)" />
      <span
        v-if="interactionCount > 0 || openQuestions > 0"
        class="inline-flex items-center gap-1 text-tiny tabular-nums"
        :class="openQuestions > 0 ? 'font-semibold text-warn' : 'text-muted'"
        :title="openQuestions > 0 ? `${openQuestions} ${openQuestions === 1 ? 'pergunta pendente' : 'perguntas pendentes'}` : `${interactionCount} interações`"
        data-testid="card-interactions"
      >
        <svg viewBox="0 0 16 16" class="size-3 shrink-0" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path :d="CHAT_BUBBLE_PATH" />
        </svg>
        <span>{{ interactionCount }}</span>
        <span v-if="openQuestions > 0">· {{ openQuestions }}?</span>
      </span>
      <span class="ml-auto shrink-0" :title="ownerName">
        <UserAvatar v-if="isUnassigned(item)" :size="AVATAR_SIZE" />
        <AgentPortrait v-else :who="item.who" :size="AVATAR_SIZE" />
      </span>
    </div>

    <div v-if="progress" class="flex flex-col gap-1" data-testid="card-progress">
      <p v-if="progress.showText" class="hidden justify-between text-meta text-muted xl:flex">
        <span>{{ item.children?.total }} micro-tarefas · {{ progress.filled }} feitas</span>
        <span>{{ percent }}%</span>
      </p>
      <p v-else-if="progress.statusText" class="truncate text-tiny text-muted">{{ progress.statusText }}</p>
      <div class="flex gap-0.75">
        <span
          v-for="segment in progress.total"
          :key="segment"
          class="h-1 flex-1 rounded-sm"
          :class="segment <= progress.filled ? progress.fillClass : 'bg-surface-2'"
        />
      </div>
    </div>
  </article>
</template>
