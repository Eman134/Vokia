<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import UiBadge from '../../ui/UiBadge.vue'
import UiSketchBox from '../../ui/UiSketchBox.vue'
import UiMarker from '../../ui/UiMarker.vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import { useFeed } from '../team/useFeed'
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
const CARD_HOVER_CLASSES =
  'transition-[border-color,box-shadow,background-color] duration-200 ease-out hover:border-ink/35 hover:shadow-paper motion-reduce:transition-none'
const FULL_CARD_CLASSES =
  `group relative flex min-w-0 cursor-pointer flex-col gap-1 rounded-sm border border-line bg-surface px-2 py-2 lg:gap-1.5 lg:px-3 lg:py-2.5 ${CARD_HOVER_CLASSES}`
const COMPACT_CARD_CLASSES =
  `group relative flex min-w-0 cursor-pointer select-none flex-col gap-1 rounded-sm border border-line bg-surface px-2 py-1.5 ${CARD_HOVER_CLASSES}`
const COMPACT_EXPAND_CLASSES =
  'grid grid-rows-[0fr] opacity-0 pointer-events-none transition-[grid-template-rows,opacity] duration-200 ease-[cubic-bezier(0.22,1,0.36,1)] group-hover:grid-rows-[1fr] group-hover:opacity-100 group-hover:pointer-events-auto group-focus-within:grid-rows-[1fr] group-focus-within:opacity-100 group-focus-within:pointer-events-auto motion-reduce:transition-none'
const SELECTED_CARD_CLASSES = 'border-accent bg-accent-soft'
const DRAGGING_GHOST_CLASSES = 'opacity-35 border-dashed border-graphite bg-surface-2 pointer-events-none select-none'
const FLOATING_CARD_CLASSES = 'rotate-3 scale-[1.02] shadow-paper ring-1 ring-ink/15 pointer-events-none select-none'
const CHECKBOX_BASE_CLASSES = `size-4 shrink-0 cursor-pointer items-center justify-center rounded-sm border transition-colors ${INK_FOCUS_CLASSES}`
const CHECKBOX_ON_CLASSES = 'inline-flex border-accent bg-accent text-accent-ink'
const CHECKBOX_OFF_CLASSES = 'hidden border-line bg-surface text-transparent hover:border-ink group-hover:inline-flex group-focus-within:inline-flex'
const ID_CLASSES = 'shrink-0 rounded-sm bg-surface-2 px-1.5 py-0.5 font-mono text-tiny font-bold tracking-wide text-ink'
const TAG_CLASSES = 'max-w-[110px] truncate rounded-sm border border-line bg-surface-2 px-1.5 py-0.5 text-tiny text-muted'
const PAPERCLIP_PATH = 'M11.5 4.5 6.3 9.7a2 2 0 1 1-2.8-2.8l5.6-5.6a3.2 3.2 0 0 1 4.5 4.5l-5.7 5.7a4.4 4.4 0 0 1-6.2-6.2l5.2-5.2'
const CHECK_PATH = 'M3.5 8.5 6.5 11.5 12.5 5'
const QUEUED_TEXT = 'Na fila de triagem…'
const GENERATING_TEXT = 'Escrevendo o plano…'
const COPY_ID_LABEL = 'Copiar identificador'
const MORE_BUTTON_CLASSES = `inline-flex size-5 shrink-0 cursor-pointer items-center justify-center rounded-sm text-muted transition-opacity hover:bg-hover hover:text-ink ${INK_FOCUS_CLASSES}`
const MENU_POPOVER_CLASSES = 'absolute right-0 top-full z-30 mt-1 w-44 rounded-md border border-line bg-surface p-1 shadow-paper'
const MENU_ITEM_CLASSES = `flex w-full cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-left text-caption text-ink hover:bg-hover ${INK_FOCUS_CLASSES}`

const feed = useFeed()
const roster = useRoster()
const menuOpen = ref(false)
const menuRef = ref<HTMLElement | null>(null)
const itemUsage = useItemUsage(() => props.item.id)
const usageText = computed(() => formatItemUsage(itemUsage.value))
const displayId = computed(() => props.item.publicId ?? props.item.id)
const visibleTags = computed(() => props.item.tags.slice(0, MAX_VISIBLE_TAGS))
const extraTagsCount = computed(() => Math.max(0, props.item.tags.length - MAX_VISIBLE_TAGS))
const descriptionContent = computed(() => extractAttachments(props.item.description ?? ''))
const compactDescription = computed(() => descriptionContent.value.text.trim())
const attachmentCount = computed(() => descriptionContent.value.attachments.length)
const isCompact = computed(() => props.density === 'compact')

const chip = computed(
  () => COLUMN_CHIP[props.column] ?? { label: STATE_CHIP[props.state], status: STATE_BADGE[props.state] },
)
const isFeature = computed(() => props.item.type === 'feature')
const isAutoClosed = computed(() => props.column === DONE_COLUMN && props.item.closedBy === CLOSED_BY_AUTOMATIC)
const ownerName = computed(() => (isUnassigned(props.item) ? UNASSIGNED_TEXT : (roster.profileOf(props.item.who)?.name ?? props.item.who)))

const action = computed(() => cardActionFor(props.item, props.column, props.state))

const progress = computed(() => {
  const { children, triageStep, criteria } = props.item
  if (props.column === DONE_COLUMN) {
    const hasChildren = children !== undefined && children.total > 0
    const criteriaTotal = criteria?.length ?? 0
    const total = Math.max(hasChildren ? children.total : criteriaTotal, DONE_SEGMENTS)
    const text = hasChildren
      ? `${children.total} micro-tarefas · ${children.total} feitas`
      : criteriaTotal > 0
        ? `${criteriaTotal} ${criteriaTotal === 1 ? 'critério' : 'critérios'} · ${criteriaTotal} ${criteriaTotal === 1 ? 'feito' : 'feitos'}`
        : undefined
    return { total, filled: total, fillClass: 'bg-ok', text, statusText: undefined }
  }
  if (children && children.total > 0) {
    const filled = Math.min(children.done, children.total)
    return {
      total: children.total,
      filled,
      fillClass: 'bg-ok',
      text: `${children.total} micro-tarefas · ${filled} feitas`,
      statusText: undefined,
    }
  }
  if (props.state === 'triagem') {
    return { total: TRIAGE_STEP_COUNT, filled: triageStep ?? 0, fillClass: 'bg-accent', text: undefined, statusText: props.planLine }
  }
  if (props.state === 'gerando') {
    return { total: TRIAGE_STEP_COUNT, filled: 1, fillClass: 'bg-accent', text: undefined, statusText: props.planLine ?? GENERATING_TEXT }
  }
  if (props.queued) {
    return { total: TRIAGE_STEP_COUNT, filled: 0, fillClass: 'bg-accent', text: undefined, statusText: QUEUED_TEXT }
  }
  if (criteria && criteria.length > 0) {
    const filled = criteria.filter((criterion) => criterion.done).length
    const total = criteria.length
    return {
      total,
      filled,
      fillClass: 'bg-ok',
      text: `${total} ${total === 1 ? 'critério' : 'critérios'} · ${filled} ${filled === 1 ? 'feito' : 'feitos'}`,
      statusText: undefined,
    }
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

function closeMenu(): void {
  menuOpen.value = false
}

function toggleMenu(): void {
  menuOpen.value = !menuOpen.value
}

async function copyIdentifier(): Promise<void> {
  closeMenu()
  const identifier = displayId.value
  try {
    await navigator.clipboard.writeText(identifier)
    feed.notify(`Identificador ${identifier} copiado.`)
  } catch {
    feed.notify(`Não foi possível copiar ${identifier}.`)
  }
}

function onWindowPointerDown(event: PointerEvent): void {
  if (!menuOpen.value) return
  if (event.target instanceof Node && menuRef.value?.contains(event.target)) return
  closeMenu()
}

function onWindowKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape' && menuOpen.value) {
    event.stopPropagation()
    closeMenu()
  }
}

watch(menuOpen, (open) => {
  if (open) {
    window.addEventListener('pointerdown', onWindowPointerDown)
    window.addEventListener('keydown', onWindowKeydown)
  } else {
    window.removeEventListener('pointerdown', onWindowPointerDown)
    window.removeEventListener('keydown', onWindowKeydown)
  }
})

onUnmounted(() => {
  window.removeEventListener('pointerdown', onWindowPointerDown)
  window.removeEventListener('keydown', onWindowKeydown)
})

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
      <UiBadge v-if="isAutoClosed" status="trabalhando" :label="AUTO_CLOSED_LABEL" />
      <div v-if="!floating" class="ml-auto flex shrink-0 items-center gap-1.5">
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
        <div ref="menuRef" class="relative shrink-0" @pointerdown.stop @click.stop>
          <button
            type="button"
            :aria-expanded="menuOpen"
            aria-haspopup="menu"
            :aria-label="`Ações do card ${displayId}`"
            title="Mais ações"
            data-testid="card-more-actions"
            :class="[
              MORE_BUTTON_CLASSES,
              menuOpen ? 'opacity-100 bg-hover text-ink' : 'opacity-0 group-hover:opacity-100 group-focus-within:opacity-100',
            ]"
            @click.stop="toggleMenu()"
          >
            <svg viewBox="0 0 16 16" class="size-3.5" fill="currentColor" aria-hidden="true">
              <circle cx="3.5" cy="8" r="1.2" />
              <circle cx="8" cy="8" r="1.2" />
              <circle cx="12.5" cy="8" r="1.2" />
            </svg>
          </button>
          <div v-if="menuOpen" role="menu" :class="MENU_POPOVER_CLASSES">
            <button
              type="button"
              role="menuitem"
              data-testid="card-copy-id"
              :class="MENU_ITEM_CLASSES"
              @click.stop="void copyIdentifier()"
            >
              <svg viewBox="0 0 16 16" class="size-3.5 shrink-0 text-muted" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <rect x="5.5" y="5.5" width="8" height="8" rx="1.2" />
                <path d="M10.5 5.5V3.7A1.2 1.2 0 0 0 9.3 2.5H3.7A1.2 1.2 0 0 0 2.5 3.7v5.6a1.2 1.2 0 0 0 1.2 1.2h1.8" />
              </svg>
              <span>{{ COPY_ID_LABEL }}</span>
            </button>
          </div>
        </div>
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
      :class="COMPACT_EXPAND_CLASSES"
      data-testid="card-compact-description"
    >
      <div class="min-h-0 overflow-hidden">
        <p class="line-clamp-3 text-meta leading-[1.35] text-muted">{{ compactDescription }}</p>
      </div>
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
      :class="COMPACT_EXPAND_CLASSES"
      data-testid="card-compact-actions"
    >
      <div class="min-h-0 overflow-hidden">
        <div class="flex items-center gap-1.5 pt-0.5">
          <BacklogPlanActions :action="action" @action="emit('action', $event)" />
        </div>
      </div>
    </div>

    <div v-if="!isCompact" class="mt-auto flex items-center gap-1.5 pt-0.5 lg:gap-2">
      <BacklogPlanActions v-if="action" :action="action" class="hidden lg:flex" @action="emit('action', $event)" />
      <span class="ml-auto shrink-0" :title="ownerName">
        <UserAvatar v-if="isUnassigned(item)" :size="AVATAR_SIZE" />
        <AgentPortrait v-else :who="item.who" :size="AVATAR_SIZE" />
      </span>
    </div>

    <div v-if="progress" class="flex flex-col gap-1" data-testid="card-progress">
      <p v-if="progress.text" class="hidden justify-between text-meta text-muted xl:flex">
        <span>{{ progress.text }}</span>
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
