<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import UiCreateButton from '../../ui/UiCreateButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import UiHandLine from '../../ui/UiHandLine.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import UiSkeleton from '../../ui/UiSkeleton.vue'
import { useNavigation } from '../../shell/useNavigation'
import { useProjects } from '../projects/useProjects'
import { useRoster } from '../team/useRoster'
import TaskCard from './TaskCard.vue'
import BatchTriageBar from './BatchTriageBar.vue'
import SprintManagerModal from './SprintManagerModal.vue'
import {
  DEFAULT_INTERACTION_FILTER,
  DEFAULT_TIME_FILTER,
  effectiveLastActivityAt,
  INTERACTION_FILTER_OPTIONS,
  matchesInteractionFilter,
  matchesSprintFilter,
  matchesTimeFilter,
  SPRINT_ALL_FILTER,
  SPRINT_NONE_FILTER,
  TIME_FILTER_OPTIONS,
  type InteractionFilter,
  type TimeFilter,
} from './backlogFilters'
import {
  applySelectionClick,
  createEmptySelection,
  reconcileSelection,
} from './batchSelection'
import { colorVar } from './backlogLabels'
import {
  ALL_FEATURES_LABEL,
  ALL_FEATURES_VALUE,
  TASKS_TITLE,
  COLUMN_META,
  EMPTY_BOARD_TEXT,
  EMPTY_BOARD_TITLE,
  EMPTY_COLUMN_TEXT,
  LOAD_ERROR_PREFIX,
  NEW_REQUEST_LABEL,
  SPRINT_ALL_LABEL,
  SPRINT_NONE_LABEL,
  SPRINTS_BUTTON_LABEL,
} from './tasksPageCopy'
import {
  COLUMN_LABEL,
  DEFAULT_COLUMN,
  isUnassigned,
  READY_COLUMN,
  type TaskCardAction,
  type BacklogColumn,
  type BacklogItem,
  type BacklogState,
  type KanbanStage,
} from './backlogTypes'
import { useFeed } from '../team/useFeed'
import { useBacklog } from './useBacklog'
import { PLAN_TAB_BY_ACTION, useItemPlan } from './useItemPlan'
import { useRunItem } from './useRunItem'
import { useTriage } from './useTriage'

// Do md para cima a página não rola: cada coluna rola sozinha, e o cabeçalho e o "Novo pedido" ficam à vista.
// A folga da barra de ação sai da página e vai para o fim de cada coluna.
const PAGE_CLASSES = `flex min-h-0 min-w-0 flex-1 flex-col gap-4 overflow-y-auto md:overflow-hidden ${PAGE_INSET_CLASSES} md:pb-0`
const FILTER_ROW_CLASSES = 'flex flex-wrap items-center gap-2'
const SEARCH_INPUT_CLASSES =
  'h-8 w-full min-w-[200px] flex-1 rounded-md border border-line bg-surface px-2.5 text-caption text-ink placeholder:text-muted focus:border-ink focus:outline-none sm:max-w-xs'
const FILTER_SELECT_CLASSES =
  'h-8 cursor-pointer rounded-md border border-line bg-surface px-2 text-caption text-ink focus:border-ink focus:outline-none'
const COLUMN_HEADER_CLASSES = 'sticky top-0 z-10 flex items-center justify-between gap-1 bg-bg pb-1'
const COLUMN_TITLE_CLASSES = 'relative flex min-w-0 w-fit items-baseline gap-1.5 whitespace-nowrap pb-1 text-small lg:text-card-title text-ink'
const COLUMN_COUNT_CLASSES = 'text-tiny lg:text-caption text-muted'
const COLUMN_UNDERLINE_CLASSES = 'pointer-events-none absolute inset-x-0 bottom-0 h-1.5'
const EMPTY_COLUMN_CLASSES = 'font-hand text-annotation text-faint'
const GRID_SCROLL_CLASSES = 'min-w-0 md:min-h-0 md:flex-1'
const GRID_CLASSES = 'grid min-w-0 grid-cols-1 items-start gap-1.5 lg:gap-2.5 md:h-full md:grid-cols-5 md:items-stretch'
const COLUMN_CLASSES = 'flex min-w-0 flex-col gap-1.5 lg:gap-2.5 md:min-h-0 md:overflow-y-auto md:pb-bar-clearance md:pr-0.5'
const BOARD_AREA_CLASSES = 'flex min-h-0 flex-1 flex-col'
const EMPTY_BOARD_CLASSES = 'flex flex-1 items-center justify-center'

const STAGE_COLUMN: Record<KanbanStage, BacklogColumn> = {
  ready: 'Ready',
  progress: 'In progress',
  review: 'In review',
  done: 'Done',
}

const backlog = useBacklog()
const itemPlan = useItemPlan()
const runItem = useRunItem()
const triage = useTriage()
const navigation = useNavigation()
const projectsStore = useProjects()
const roster = useRoster()
const feed = useFeed()
const projectName = computed(() => projectsStore.selected?.name ?? '')

function tryOpenPendingCard(): void {
  const cardId = navigation.pendingCardId
  if (!cardId || backlog.loading) return
  if (!backlog.items.some((item) => item.id === cardId)) return
  navigation.takePendingCardId()
  onOpenCard(cardId)
}

watch([() => navigation.pendingCardId, () => backlog.items, () => backlog.loading], tryOpenPendingCard, {
  immediate: true,
})

function columnOf(item: BacklogItem): BacklogColumn {
  return item.column ?? (item.stage ? STAGE_COLUMN[item.stage] : DEFAULT_COLUMN)
}

type ColumnSortKey =
  | 'default'
  | 'title-asc'
  | 'title-desc'
  | 'id-asc'
  | 'id-desc'
  | 'priority'
  | 'progress'
  | 'activity-desc'
  | 'interactions-desc'

const SORT_OPTIONS: { value: ColumnSortKey; label: string }[] = [
  { value: 'default', label: 'Padrão' },
  { value: 'id-desc', label: 'Mais recentes' },
  { value: 'activity-desc', label: 'Atividade recente' },
  { value: 'interactions-desc', label: 'Mais interações' },
  { value: 'id-asc', label: 'ID (A–Z)' },
  { value: 'title-asc', label: 'Título (A–Z)' },
  { value: 'title-desc', label: 'Título (Z–A)' },
  { value: 'priority', label: 'Prioridade' },
  { value: 'progress', label: 'Progresso' },
]

const ALL_FILTER_VALUE = '__all__'
const UNASSIGNED_FILTER_VALUE = '__unassigned__'

const dismissedError = ref(false)
const filtersOpen = ref(false)
const sprintManagerOpen = ref(false)
const openSortColumn = ref<BacklogColumn | null>(null)

function onSortMenuPointerDown(e: PointerEvent): void {
  if (openSortColumn.value === null) return
  if (e.target instanceof Element && e.target.closest('[data-sort-menu]')) return
  openSortColumn.value = null
}

function onWindowKeydown(e: KeyboardEvent): void {
  if (e.key !== 'Escape') return
  if (draggingId.value !== null) {
    clearDragState()
    return
  }
  if (openSortColumn.value !== null) {
    openSortColumn.value = null
    return
  }
  if (filtersOpen.value) {
    filtersOpen.value = false
    return
  }
  if (selection.value.selected.size > 0 && backlog.openId === null && itemPlan.dialogItemId === null) {
    batchBarRef.value?.restoreFocusIfInside()
    clearSelection()
  }
}

onMounted(() => {
  window.addEventListener('pointerdown', onSortMenuPointerDown)
  window.addEventListener('keydown', onWindowKeydown)
})
onUnmounted(() => {
  window.removeEventListener('pointerdown', onSortMenuPointerDown)
  window.removeEventListener('keydown', onWindowKeydown)
})
const searchQuery = ref('')
const featureFilter = ref<string>(ALL_FEATURES_VALUE)
const ownerFilter = ref<string>(ALL_FILTER_VALUE)
const tagFilter = ref<string>(ALL_FILTER_VALUE)
const typeFilter = ref<string>(ALL_FILTER_VALUE)
const sprintFilter = ref<string>(SPRINT_ALL_FILTER)
const timeFilter = ref<TimeFilter>(DEFAULT_TIME_FILTER)
const interactionFilter = ref<InteractionFilter>(DEFAULT_INTERACTION_FILTER)

const activeFilterCount = computed(() => {
  let n = 0
  if (featureFilter.value !== ALL_FEATURES_VALUE) n += 1
  if (ownerFilter.value !== ALL_FILTER_VALUE) n += 1
  if (tagFilter.value !== ALL_FILTER_VALUE) n += 1
  if (typeFilter.value !== ALL_FILTER_VALUE) n += 1
  if (sprintFilter.value !== SPRINT_ALL_FILTER) n += 1
  if (timeFilter.value !== DEFAULT_TIME_FILTER) n += 1
  if (interactionFilter.value !== DEFAULT_INTERACTION_FILTER) n += 1
  return n
})
const columnSort = ref<Record<BacklogColumn, ColumnSortKey>>({
  Backlog: 'default',
  Ready: 'default',
  'In progress': 'default',
  'In review': 'default',
  Done: 'default',
})

const featureOptions = computed(() => [
  { label: ALL_FEATURES_LABEL, value: ALL_FEATURES_VALUE },
  ...[...new Set(backlog.items.map((item) => item.feature).filter((feature): feature is string => Boolean(feature)))]
    .sort((a, b) => a.localeCompare(b, 'pt-BR'))
    .map((feature) => ({ label: feature, value: feature })),
])

const ownerOptions = computed(() => {
  const seen = new Map<string, string>()
  for (const item of backlog.items) {
    if (!item.parent && !isUnassigned(item) && !seen.has(item.who)) {
      seen.set(item.who, roster.profileOf(item.who)?.name ?? item.who)
    }
  }
  return [...seen.entries()]
    .sort((a, b) => a[1].localeCompare(b[1], 'pt-BR'))
    .map(([value, label]) => ({ value, label }))
})

const tagOptions = computed(() =>
  [...new Set(backlog.items.filter((item) => !item.parent).flatMap((item) => item.tags))].sort((a, b) =>
    a.localeCompare(b, 'pt-BR'),
  ),
)

const hasActiveFilters = computed(
  () =>
    searchQuery.value.trim() !== '' ||
    featureFilter.value !== ALL_FEATURES_VALUE ||
    ownerFilter.value !== ALL_FILTER_VALUE ||
    tagFilter.value !== ALL_FILTER_VALUE ||
    typeFilter.value !== ALL_FILTER_VALUE ||
    sprintFilter.value !== SPRINT_ALL_FILTER ||
    timeFilter.value !== DEFAULT_TIME_FILTER ||
    interactionFilter.value !== DEFAULT_INTERACTION_FILTER,
)

function clearFilters(): void {
  searchQuery.value = ''
  featureFilter.value = ALL_FEATURES_VALUE
  ownerFilter.value = ALL_FILTER_VALUE
  tagFilter.value = ALL_FILTER_VALUE
  typeFilter.value = ALL_FILTER_VALUE
  sprintFilter.value = SPRINT_ALL_FILTER
  timeFilter.value = DEFAULT_TIME_FILTER
  interactionFilter.value = DEFAULT_INTERACTION_FILTER
}

watch(
  () => projectsStore.selected?.id,
  () => {
    clearFilters()
    sprintManagerOpen.value = false
  },
)

const summarySprint = computed(() => {
  if (sprintFilter.value !== SPRINT_ALL_FILTER && sprintFilter.value !== SPRINT_NONE_FILTER) {
    return backlog.sprintById(sprintFilter.value) ?? null
  }
  return backlog.activeSprint
})

const summarySprintPercent = computed(() => {
  const sprint = summarySprint.value
  if (!sprint || sprint.totalItems <= 0) return 0
  return Math.round((sprint.doneItems / sprint.totalItems) * 100)
})

const topLevelItems = computed(() => backlog.items.filter((item) => !item.parent))

const visibleItems = computed(() => {
  const query = searchQuery.value.trim().toLowerCase()
  const activeSprint = backlog.activeSprint
  return topLevelItems.value.filter((item) => {
    if (featureFilter.value !== ALL_FEATURES_VALUE && item.feature !== featureFilter.value) return false
    if (ownerFilter.value === UNASSIGNED_FILTER_VALUE && !isUnassigned(item)) return false
    if (ownerFilter.value !== ALL_FILTER_VALUE && ownerFilter.value !== UNASSIGNED_FILTER_VALUE && item.who !== ownerFilter.value) return false
    if (tagFilter.value !== ALL_FILTER_VALUE && !item.tags.includes(tagFilter.value)) return false
    if (typeFilter.value !== ALL_FILTER_VALUE && item.type !== typeFilter.value) return false
    if (!matchesSprintFilter(item, sprintFilter.value)) return false
    if (!matchesTimeFilter(item, timeFilter.value, activeSprint)) return false
    if (!matchesInteractionFilter(item, interactionFilter.value)) return false
    if (!query) return true
    const ownerName = isUnassigned(item) ? '' : (roster.profileOf(item.who)?.name ?? item.who)
    return (
      item.id.toLowerCase().includes(query) ||
      (item.publicId?.toLowerCase().includes(query) ?? false) ||
      item.title.toLowerCase().includes(query) ||
      (item.feature?.toLowerCase().includes(query) ?? false) ||
      ownerName.toLowerCase().includes(query) ||
      item.tags.some((tag) => tag.toLowerCase().includes(query))
    )
  })
})

function priorityRank(priority: string | undefined): number {
  const p = priority?.trim().toLowerCase() ?? ''
  if (p === 'alta' || p === 'high' || p === 'p0' || p === 'p1') return 0
  if (p === 'media' || p === 'média' || p === 'medium' || p === 'p2') return 1
  if (p === 'baixa' || p === 'low' || p === 'p3') return 2
  return 3
}

function progressRatio(item: BacklogItem): number {
  if (!item.children || item.children.total === 0) return -1
  return item.children.done / item.children.total
}

function sortCards(cards: BacklogItem[], key: ColumnSortKey): BacklogItem[] {
  if (key === 'default') return cards
  const copy = [...cards]
  copy.sort((a, b) => {
    const aId = a.publicId ?? a.id
    const bId = b.publicId ?? b.id
    if (key === 'title-asc') return a.title.localeCompare(b.title, 'pt-BR')
    if (key === 'title-desc') return b.title.localeCompare(a.title, 'pt-BR')
    if (key === 'id-asc') return aId.localeCompare(bId, 'pt-BR', { numeric: true })
    if (key === 'id-desc') return bId.localeCompare(aId, 'pt-BR', { numeric: true })
    if (key === 'priority') return priorityRank(a.priority) - priorityRank(b.priority)
    if (key === 'progress') return progressRatio(b) - progressRatio(a)
    if (key === 'activity-desc') return (effectiveLastActivityAt(b) ?? 0) - (effectiveLastActivityAt(a) ?? 0)
    if (key === 'interactions-desc') return (b.interactionCount ?? 0) - (a.interactionCount ?? 0)
    return 0
  })
  return copy
}

const isEmptyBoard = computed(() => !backlog.loading && !backlog.loadError && backlog.items.length === 0 && projectName.value !== '')
watch(featureOptions, (options) => {
  if (!options.some((option) => option.value === featureFilter.value)) featureFilter.value = ALL_FEATURES_VALUE
})

function onCardAction(itemId: string, action: TaskCardAction): void {
  if (backlog.openId !== null) return
  if (action === 'iniciar') {
    void runItem.run(itemId)
    return
  }
  itemPlan.openDialog(itemId, PLAN_TAB_BY_ACTION[action])
}

function onOpenCard(itemId: string): void {
  itemPlan.closeDialog()
  backlog.open(itemId)
}

const columns = computed(() =>
  COLUMN_META.map((column) => ({
    ...column,
    cards: sortCards(
      visibleItems.value.filter((item) => columnOf(item) === column.name),
      columnSort.value[column.name],
    ),
  })),
)

const NON_TRIAGEABLE_STATES: readonly BacklogState[] = ['gerando', 'triagem', 'proposto']
const batchBarRef = ref<InstanceType<typeof BatchTriageBar> | null>(null)
const selection = ref(createEmptySelection())

function canSelectForTriage(item: BacklogItem): boolean {
  return !NON_TRIAGEABLE_STATES.includes(itemPlan.stateFor(item)) && !triage.isQueued(item.id)
}

const backlogCardIds = computed(
  () =>
    columns.value
      .find((column) => column.name === DEFAULT_COLUMN)
      ?.cards.filter(canSelectForTriage)
      .map((item) => item.id) ?? [],
)

function clearSelection(): void {
  selection.value = createEmptySelection()
}

function onToggleSelect(itemId: string, shiftKey: boolean): void {
  selection.value = applySelectionClick({
    selected: selection.value.selected,
    anchorId: selection.value.anchorId,
    orderedIds: backlogCardIds.value,
    targetId: itemId,
    shiftKey,
  })
}

function onBatchTriage(): void {
  const orderedSelected = backlogCardIds.value.filter((id) => selection.value.selected.has(id))
  if (orderedSelected.length === 0) return
  triage.enqueueBatch(orderedSelected)
  clearSelection()
}

watch(backlogCardIds, (ids) => {
  selection.value = reconcileSelection(selection.value, ids)
})

watch(
  () => projectsStore.selected?.id,
  () => {
    dismissedError.value = false
    clearSelection()
    clearDragState()
  },
)

const DRAG_THRESHOLD_PX = 6
const draggingId = ref<string | null>(null)
const dragOverColumn = ref<BacklogColumn | null>(null)
const dragOverIndex = ref<number | null>(null)
const dragPos = ref({ x: 0, y: 0 })
const dragCardSize = ref({ width: 260, height: 80, offsetX: 40, offsetY: 20 })
let pendingDrag: {
  item: BacklogItem
  startX: number
  startY: number
  rect: DOMRect
} | null = null
let suppressNextClick = false

const draggingItem = computed(() =>
  draggingId.value ? backlog.items.find((item) => item.id === draggingId.value) ?? null : null,
)

function setBodyDragCursor(active: boolean): void {
  document.body.style.cursor = active ? 'grabbing' : ''
  document.body.style.userSelect = active ? 'none' : ''
}

function clearDragState(): void {
  pendingDrag = null
  draggingId.value = null
  dragOverColumn.value = null
  dragOverIndex.value = null
  setBodyDragCursor(false)
  window.removeEventListener('pointermove', onWindowPointerMove)
  window.removeEventListener('pointerup', onWindowPointerUp)
  window.removeEventListener('pointercancel', onWindowPointerUp)
}

onUnmounted(() => {
  clearDragState()
})

function canDropOnColumn(item: BacklogItem, target: BacklogColumn): { allowed: boolean; reason?: string } {
  const source = columnOf(item)
  if (source === target) return { allowed: true }
  if (item.error) return { allowed: false, reason: 'Card com erro não pode ser movido.' }
  return { allowed: true }
}

function columnDropAllowed(column: BacklogColumn): boolean {
  if (!draggingItem.value) return false
  return canDropOnColumn(draggingItem.value, column).allowed
}

function onCardPointerDown(event: PointerEvent, item: BacklogItem): void {
  if (event.button !== 0 || item.error) return
  const target = event.target as HTMLElement | null
  if (target?.closest('button, input, select, textarea, a, [role="checkbox"]')) return
  const cardEl = event.currentTarget as HTMLElement | null
  if (!cardEl) return
  pendingDrag = {
    item,
    startX: event.clientX,
    startY: event.clientY,
    rect: cardEl.getBoundingClientRect(),
  }
  window.addEventListener('pointermove', onWindowPointerMove)
  window.addEventListener('pointerup', onWindowPointerUp)
  window.addEventListener('pointercancel', onWindowPointerUp)
}

function hitTestDropTarget(clientX: number, clientY: number): { column: BacklogColumn; index: number } | null {
  const cols = document.querySelectorAll<HTMLElement>('[data-drop-column]')
  let matchedCol: HTMLElement | null = null
  for (const col of cols) {
    const rect = col.getBoundingClientRect()
    if (clientX >= rect.left && clientX <= rect.right && clientY >= rect.top && clientY <= rect.bottom) {
      matchedCol = col
      break
    }
  }
  if (!matchedCol) return null
  const colName = matchedCol.dataset.dropColumn as BacklogColumn | undefined
  if (!colName) return null

  const cards = [...matchedCol.querySelectorAll<HTMLElement>('[data-card-id]')]
    .filter((el) => el.dataset.cardId !== draggingId.value)
  let index = cards.length
  for (let i = 0; i < cards.length; i += 1) {
    const rect = cards[i]!.getBoundingClientRect()
    if (clientY < rect.top + rect.height / 2) {
      index = i
      break
    }
  }
  return { column: colName, index }
}

function onWindowPointerMove(event: PointerEvent): void {
  if (pendingDrag && draggingId.value === null) {
    const dist = Math.hypot(event.clientX - pendingDrag.startX, event.clientY - pendingDrag.startY)
    if (dist < DRAG_THRESHOLD_PX) return
    draggingId.value = pendingDrag.item.id
    dragCardSize.value = {
      width: pendingDrag.rect.width,
      height: pendingDrag.rect.height,
      offsetX: event.clientX - pendingDrag.rect.left,
      offsetY: event.clientY - pendingDrag.rect.top,
    }
    suppressNextClick = true
    setBodyDragCursor(true)
  }
  if (draggingId.value === null) return
  dragPos.value = { x: event.clientX, y: event.clientY }
  const hit = hitTestDropTarget(event.clientX, event.clientY)
  dragOverColumn.value = hit?.column ?? null
  dragOverIndex.value = hit?.index ?? null
}

function onWindowPointerUp(): void {
  const item = draggingItem.value
  const targetColumn = dragOverColumn.value
  const targetIndex = dragOverIndex.value
  clearDragState()
  if (suppressNextClick) {
    requestAnimationFrame(() => {
      suppressNextClick = false
    })
  }
  if (!item || !targetColumn) return
  void applyDrop(item, targetColumn, targetIndex)
}

function onOpenCardSafe(itemId: string): void {
  if (suppressNextClick) return
  onOpenCard(itemId)
}

function showDropGhostAt(column: BacklogColumn, slotIndex: number): boolean {
  if (!draggingItem.value || dragOverColumn.value !== column || dragOverIndex.value !== slotIndex) {
    return false
  }
  if (!columnDropAllowed(column)) return false
  // Na mesma coluna, se não mudou de posição em relação ao card original, ainda mostra o fantasma na área de drop apenas quando o card saiu da vaga ou em outra coluna.
  return true
}

function cardsForRender(column: BacklogColumn, cards: BacklogItem[]): BacklogItem[] {
  if (dragOverColumn.value !== column || draggingId.value === null) return cards
  return cards.filter((c) => c.id !== draggingId.value)
}

async function applyDrop(item: BacklogItem, targetColumn: BacklogColumn, targetIndex: number | null): Promise<void> {
  const sourceColumn = columnOf(item)
  if (sourceColumn === targetColumn) {
    if (hasActiveFilters.value) {
      feed.notify('Limpe os filtros para reordenar os cards da coluna.')
      return
    }
    const currentOrder = columns.value.find((c) => c.name === targetColumn)?.cards.map((c) => c.id) ?? []
    const withoutMoved = currentOrder.filter((id) => id !== item.id)
    const insertAt = targetIndex !== null ? Math.max(0, Math.min(targetIndex, withoutMoved.length)) : withoutMoved.length
    withoutMoved.splice(insertAt, 0, item.id)
    if (withoutMoved.every((id, i) => id === currentOrder[i])) return
    columnSort.value[targetColumn] = 'default'
    backlog.reorderColumn(targetColumn, withoutMoved)
    return
  }

  const check = canDropOnColumn(item, targetColumn)
  if (!check.allowed) {
    if (check.reason) feed.notify(check.reason)
    return
  }

  const state = itemPlan.stateFor(item)
  if (state === 'triagem' || state === 'gerando') {
    itemPlan.cancel(item.id)
  }

  if (sourceColumn === 'In progress' || runItem.isRunning(item.id)) {
    await runItem.stop(item.id)
    backlog.moveTo(item.id, targetColumn)
    return
  }

  if (targetColumn === 'In progress') {
    if (state === 'proposto') {
      itemPlan.accept(item.id)
    }
    if (runItem.canRun(item.id) && !runItem.hasPendingDecision(item.id)) {
      void runItem.run(item.id)
    } else {
      backlog.moveTo(item.id, targetColumn)
    }
    return
  }

  if (targetColumn === READY_COLUMN) {
    if (state === 'proposto') {
      itemPlan.accept(item.id)
    } else {
      backlog.moveTo(item.id, READY_COLUMN)
    }
    return
  }

  backlog.moveTo(item.id, targetColumn)
}
</script>

<template>
  <section :class="PAGE_CLASSES">
    <UiPageHeader :title="TASKS_TITLE">
      <template #actions>
        <div class="flex items-center gap-2">
          <button
            type="button"
            :disabled="!projectsStore.selected"
            class="inline-flex h-8 cursor-pointer items-center gap-1.5 rounded-md border border-line bg-surface px-3 text-caption font-medium text-ink hover:bg-hover disabled:cursor-not-allowed disabled:opacity-45"
            data-testid="open-sprint-manager"
            @click="sprintManagerOpen = true"
          >
            {{ SPRINTS_BUTTON_LABEL }}
          </button>
          <UiCreateButton :disabled="!projectsStore.selected" @press="backlog.startRequest()">
            {{ NEW_REQUEST_LABEL }}
          </UiCreateButton>
        </div>
      </template>
    </UiPageHeader>

    <div :class="FILTER_ROW_CLASSES">
      <input
        v-model="searchQuery"
        type="search"
        :class="SEARCH_INPUT_CLASSES"
        placeholder="Buscar por ID, título, etiqueta ou responsável…"
        aria-label="Buscar cards no backlog"
        data-testid="backlog-search"
      />
      <div class="relative lg:hidden">
        <button
          type="button"
          class="inline-flex h-8 cursor-pointer items-center gap-1.5 rounded-md border border-line bg-surface px-2.5 text-caption text-ink hover:bg-hover"
          aria-label="Filtros"
          :aria-expanded="filtersOpen"
          data-testid="backlog-filters-toggle"
          @click="filtersOpen = !filtersOpen"
        >
          <svg viewBox="0 0 16 16" class="size-3.5 shrink-0" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M2 3h12l-4.5 5.2v4l-3 1.3V8.2L2 3Z" />
          </svg>
          <span>Filtros</span>
          <span v-if="activeFilterCount > 0" class="rounded-full bg-accent px-1.5 text-tiny font-semibold text-accent-ink">
            {{ activeFilterCount }}
          </span>
        </button>
        <div
          v-if="filtersOpen"
          class="absolute right-0 top-full z-30 mt-1.5 flex w-64 flex-col gap-2 rounded-md border border-line bg-surface p-3 shadow-paper"
        >
          <UiSegmented v-if="featureOptions.length > 1" v-model="featureFilter" :options="featureOptions" label="Feature" wrap />
          <select v-model="ownerFilter" :class="[FILTER_SELECT_CLASSES, 'w-full']" aria-label="Filtrar por responsável">
            <option :value="ALL_FILTER_VALUE">Responsável: todos</option>
            <option :value="UNASSIGNED_FILTER_VALUE">Sem responsável</option>
            <option v-for="owner in ownerOptions" :key="owner.value" :value="owner.value">{{ owner.label }}</option>
          </select>
          <select v-if="tagOptions.length > 0" v-model="tagFilter" :class="[FILTER_SELECT_CLASSES, 'w-full']" aria-label="Filtrar por etiqueta">
            <option :value="ALL_FILTER_VALUE">Etiqueta: todas</option>
            <option v-for="tag in tagOptions" :key="tag" :value="tag">{{ tag }}</option>
          </select>
          <select v-model="typeFilter" :class="[FILTER_SELECT_CLASSES, 'w-full']" aria-label="Filtrar por tipo">
            <option :value="ALL_FILTER_VALUE">Tipo: todos</option>
            <option value="task">Tasks</option>
            <option value="feature">Features</option>
          </select>
          <select v-model="sprintFilter" :class="[FILTER_SELECT_CLASSES, 'w-full']" aria-label="Filtrar por sprint">
            <option :value="SPRINT_ALL_FILTER">{{ SPRINT_ALL_LABEL }}</option>
            <option :value="SPRINT_NONE_FILTER">{{ SPRINT_NONE_LABEL }}</option>
            <option v-for="sprint in backlog.sprints" :key="sprint.id" :value="sprint.id">{{ sprint.name }}</option>
          </select>
          <select v-model="timeFilter" :class="[FILTER_SELECT_CLASSES, 'w-full']" aria-label="Filtrar por tempo">
            <option v-for="opt in TIME_FILTER_OPTIONS" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
          </select>
          <select v-model="interactionFilter" :class="[FILTER_SELECT_CLASSES, 'w-full']" aria-label="Filtrar por interações">
            <option v-for="opt in INTERACTION_FILTER_OPTIONS" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
          </select>
          <button
            v-if="hasActiveFilters"
            type="button"
            class="h-7 cursor-pointer rounded-md border border-line bg-surface-2 px-2 text-caption text-muted hover:text-ink"
            @click="clearFilters(); filtersOpen = false"
          >
            Limpar filtros
          </button>
        </div>
      </div>

      <div class="hidden lg:flex lg:flex-wrap lg:items-center lg:gap-2">
        <UiSegmented v-if="featureOptions.length > 1" v-model="featureFilter" :options="featureOptions" label="Feature" />
        <select v-model="ownerFilter" :class="FILTER_SELECT_CLASSES" aria-label="Filtrar por responsável" data-testid="filter-owner">
          <option :value="ALL_FILTER_VALUE">Responsável: todos</option>
          <option :value="UNASSIGNED_FILTER_VALUE">Sem responsável</option>
          <option v-for="owner in ownerOptions" :key="owner.value" :value="owner.value">
            {{ owner.label }}
          </option>
        </select>
        <select v-if="tagOptions.length > 0" v-model="tagFilter" :class="FILTER_SELECT_CLASSES" aria-label="Filtrar por etiqueta" data-testid="filter-tag">
          <option :value="ALL_FILTER_VALUE">Etiqueta: todas</option>
          <option v-for="tag in tagOptions" :key="tag" :value="tag">{{ tag }}</option>
        </select>
        <select v-model="typeFilter" :class="FILTER_SELECT_CLASSES" aria-label="Filtrar por tipo" data-testid="filter-type">
          <option :value="ALL_FILTER_VALUE">Tipo: todos</option>
          <option value="task">Tasks</option>
          <option value="feature">Features</option>
        </select>
        <select v-model="sprintFilter" :class="FILTER_SELECT_CLASSES" aria-label="Filtrar por sprint" data-testid="filter-sprint">
          <option :value="SPRINT_ALL_FILTER">{{ SPRINT_ALL_LABEL }}</option>
          <option :value="SPRINT_NONE_FILTER">{{ SPRINT_NONE_LABEL }}</option>
          <option v-for="sprint in backlog.sprints" :key="sprint.id" :value="sprint.id">{{ sprint.name }}</option>
        </select>
        <select v-model="timeFilter" :class="FILTER_SELECT_CLASSES" aria-label="Filtrar por tempo" data-testid="filter-time">
          <option v-for="opt in TIME_FILTER_OPTIONS" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
        </select>
        <select v-model="interactionFilter" :class="FILTER_SELECT_CLASSES" aria-label="Filtrar por interações" data-testid="filter-interactions">
          <option v-for="opt in INTERACTION_FILTER_OPTIONS" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
        </select>
        <button
          v-if="hasActiveFilters"
          type="button"
          class="h-8 cursor-pointer rounded-md border border-line bg-surface-2 px-2.5 text-caption text-muted hover:text-ink"
          @click="clearFilters"
        >
          Limpar
        </button>
      </div>
      <span v-if="topLevelItems.length > 0" class="ml-auto text-tiny text-muted tabular-nums">
        {{ visibleItems.length }} de {{ topLevelItems.length }}
      </span>
    </div>

    <div
      v-if="summarySprint"
      class="flex flex-wrap items-center gap-3 rounded-md border border-line bg-surface px-3 py-1.5 text-caption"
      data-testid="sprint-progress-strip"
    >
      <span class="font-semibold text-ink">{{ summarySprint.name }}</span>
      <span v-if="summarySprint.goal" class="hidden truncate text-muted sm:inline">{{ summarySprint.goal }}</span>
      <div class="ml-auto flex items-center gap-2">
        <div class="h-1.5 w-24 overflow-hidden rounded-full bg-surface-2">
          <div class="h-full rounded-full bg-ok transition-all" :style="{ width: `${summarySprintPercent}%` }" />
        </div>
        <span class="text-tiny text-muted tabular-nums">
          {{ summarySprint.doneItems }}/{{ summarySprint.totalItems }} ({{ summarySprintPercent }}%)
        </span>
      </div>
    </div>

    <UiNotice v-if="backlog.loadError && !dismissedError" :text="LOAD_ERROR_PREFIX + backlog.loadError" @close="dismissedError = true" />
    <div :class="BOARD_AREA_CLASSES">
      <div :class="GRID_SCROLL_CLASSES">
        <div
          :key="projectsStore.selected?.id"
          role="list"
          aria-label="Colunas do backlog"
          :aria-busy="backlog.loading"
          :class="[GRID_CLASSES, draggingId !== null && '[&_[data-card-id]]:pointer-events-none']"
        >
          <section
            v-for="column in columns"
            :key="column.name"
            role="listitem"
            :aria-label="COLUMN_LABEL[column.name]"
            :data-drop-column="column.name"
            :data-testid="`column-${column.name}`"
            :class="[
              COLUMN_CLASSES,
              'rounded-md transition-colors',
              dragOverColumn === column.name && (columnDropAllowed(column.name) ? 'bg-accent-soft/25 ring-2 ring-accent/40' : 'opacity-65'),
            ]"
          >
            <div :class="COLUMN_HEADER_CLASSES">
              <div :class="COLUMN_TITLE_CLASSES">
                <b>{{ COLUMN_LABEL[column.name] }}</b>
                <span :class="COLUMN_COUNT_CLASSES">{{ column.cards.length }}</span>
                <UiHandLine :class="COLUMN_UNDERLINE_CLASSES" :style="{ color: colorVar(column.colorToken) }" />
              </div>
              <div class="relative shrink-0" data-sort-menu>
                <button
                  type="button"
                  class="relative inline-flex size-6 cursor-pointer items-center justify-center rounded-sm text-muted hover:bg-hover hover:text-ink"
                  :class="columnSort[column.name] !== 'default' && 'text-accent'"
                  :title="`Ordenar ${COLUMN_LABEL[column.name]}: ${SORT_OPTIONS.find((o) => o.value === columnSort[column.name])?.label ?? 'Padrão'}`"
                  :aria-label="`Ordenar ${COLUMN_LABEL[column.name]}`"
                  :aria-expanded="openSortColumn === column.name"
                  :data-testid="`sort-${column.name}`"
                  @click="openSortColumn = openSortColumn === column.name ? null : column.name"
                >
                  <svg viewBox="0 0 16 16" class="size-3.5" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <path d="M5 3v10M5 3 2.8 5.2M5 3l2.2 2.2M11 13V3M11 13l-2.2-2.2M11 13l2.2-2.2" />
                  </svg>
                  <span
                    v-if="columnSort[column.name] !== 'default'"
                    class="absolute right-0.5 top-0.5 size-1.5 rounded-full bg-accent"
                    aria-hidden="true"
                  />
                </button>
                <div
                  v-if="openSortColumn === column.name"
                  role="menu"
                  class="absolute right-0 top-full z-30 mt-1 flex min-w-36 flex-col gap-0.5 rounded-md border border-line bg-surface p-1 shadow-paper"
                >
                  <button
                    v-for="opt in SORT_OPTIONS"
                    :key="opt.value"
                    type="button"
                    role="menuitem"
                    class="cursor-pointer rounded-sm px-2.5 py-1.5 text-left text-caption transition-colors hover:bg-hover"
                    :class="columnSort[column.name] === opt.value ? 'bg-accent-soft font-semibold text-accent' : 'text-ink'"
                    @click="columnSort[column.name] = opt.value; openSortColumn = null"
                  >
                    {{ opt.label }}
                  </button>
                </div>
              </div>
            </div>
            <span v-if="!backlog.loading && !isEmptyBoard && column.cards.length === 0" :class="EMPTY_COLUMN_CLASSES">{{ EMPTY_COLUMN_TEXT }}</span>
            <template
              v-for="(item, cardIdx) in cardsForRender(column.name, column.cards)"
              :key="item.id"
            >
              <div
                v-if="showDropGhostAt(column.name, cardIdx)"
                class="rounded-sm border-2 border-dashed border-accent/60 bg-accent-soft/35 px-2.5 py-2 text-caption text-accent/80 select-none"
                :style="{ minHeight: `${Math.min(dragCardSize.height, 84)}px` }"
                aria-hidden="true"
                data-testid="drop-ghost"
              >
                <span class="font-mono text-tiny font-bold">{{ draggingItem?.publicId ?? draggingItem?.id }}</span>
                <p class="truncate font-semibold">{{ draggingItem?.title }}</p>
              </div>
              <TaskCard
                :item="item"
                :column="column.name"
                :state="itemPlan.stateFor(item)"
                :plan-line="itemPlan.lineFor(item)"
                :proposal="itemPlan.proposalFor(item)"
                :sprint-name="backlog.sprintById(item.sprintId)?.name"
                :density="column.name === DEFAULT_COLUMN ? 'compact' : 'full'"
                :selectable="column.name === DEFAULT_COLUMN && canSelectForTriage(item)"
                :selected="column.name === DEFAULT_COLUMN && selection.selected.has(item.id)"
                :queued="triage.isQueued(item.id)"
                :dragging="draggingId === item.id"
                @pointerdown="onCardPointerDown($event, item)"
                @open="onOpenCardSafe(item.id)"
                @action="onCardAction(item.id, $event)"
                @toggle-select="onToggleSelect(item.id, $event)"
              />
            </template>
            <div
              v-if="showDropGhostAt(column.name, cardsForRender(column.name, column.cards).length)"
              class="rounded-sm border-2 border-dashed border-accent/60 bg-accent-soft/35 px-2.5 py-2 text-caption text-accent/80 select-none"
              :style="{ minHeight: `${Math.min(dragCardSize.height, 84)}px` }"
              aria-hidden="true"
              data-testid="drop-ghost"
            >
              <span class="font-mono text-tiny font-bold">{{ draggingItem?.publicId ?? draggingItem?.id }}</span>
              <p class="truncate font-semibold">{{ draggingItem?.title }}</p>
            </div>
            <UiSkeleton v-if="backlog.loading" type="card" :count="2" />
          </section>
        </div>
      </div>
      <div v-if="isEmptyBoard" :class="EMPTY_BOARD_CLASSES">
        <UiEmpty :title="EMPTY_BOARD_TITLE" :text="EMPTY_BOARD_TEXT" />
      </div>
    </div>
    <BatchTriageBar
      ref="batchBarRef"
      :selected-count="selection.selected.size"
      :queue-count="triage.queue.length"
      :batch-active="triage.isBatchActive"
      @triage="onBatchTriage"
      @clear="clearSelection"
    />
    <Teleport to="body">
      <div
        v-if="draggingItem"
        class="pointer-events-none fixed z-50"
        :style="{
          left: `${dragPos.x - dragCardSize.offsetX}px`,
          top: `${dragPos.y - dragCardSize.offsetY}px`,
          width: `${dragCardSize.width}px`,
        }"
        aria-hidden="true"
      >
        <TaskCard
          :item="draggingItem"
          :column="columnOf(draggingItem)"
          :state="itemPlan.stateFor(draggingItem)"
          :plan-line="itemPlan.lineFor(draggingItem)"
          :proposal="itemPlan.proposalFor(draggingItem)"
          :sprint-name="backlog.sprintById(draggingItem.sprintId)?.name"
          :density="columnOf(draggingItem) === DEFAULT_COLUMN ? 'compact' : 'full'"
          :queued="triage.isQueued(draggingItem.id)"
          floating
        />
      </div>
    </Teleport>
    <SprintManagerModal :open="sprintManagerOpen" @close="sprintManagerOpen = false" />
  </section>
</template>
