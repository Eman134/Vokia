<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import UiCreateButton from '../../ui/UiCreateButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import UiHandLine from '../../ui/UiHandLine.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import { useNavigation } from '../../shell/useNavigation'
import { useProjects } from '../projects/useProjects'
import { useRoster } from '../team/useRoster'
import TaskCard from './TaskCard.vue'
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
  LOADING_TEXT,
  NEW_REQUEST_LABEL,
} from './tasksPageCopy'
import {
  COLUMN_LABEL,
  DEFAULT_COLUMN,
  isUnassigned,
  type TaskCardAction,
  type BacklogColumn,
  type BacklogItem,
  type KanbanStage,
} from './backlogTypes'
import { useBacklog } from './useBacklog'
import { PLAN_TAB_BY_ACTION, useItemPlan } from './useItemPlan'
import { useRunItem } from './useRunItem'

// Do md para cima a página não rola: cada coluna rola sozinha, e o cabeçalho e o "Novo pedido" ficam à vista.
// A folga da barra de ação sai da página e vai para o fim de cada coluna.
const PAGE_CLASSES = `flex min-h-0 min-w-0 flex-1 flex-col gap-4 overflow-y-auto md:overflow-hidden ${PAGE_INSET_CLASSES} md:pb-0`
const FILTER_ROW_CLASSES = 'flex flex-wrap items-center gap-2'
const SEARCH_INPUT_CLASSES =
  'h-8 w-full min-w-[200px] flex-1 rounded-md border border-line bg-surface px-2.5 text-caption text-ink placeholder:text-muted focus:border-ink focus:outline-none sm:max-w-xs'
const FILTER_SELECT_CLASSES =
  'h-8 cursor-pointer rounded-md border border-line bg-surface px-2 text-caption text-ink focus:border-ink focus:outline-none'
const COLUMN_HEADER_CLASSES = 'sticky top-0 z-10 flex items-center justify-between gap-1 bg-bg pb-1'
const COLUMN_SORT_SELECT_CLASSES =
  'h-5.5 max-w-[74px] xl:max-w-[115px] shrink-0 cursor-pointer rounded-sm border border-line bg-surface px-1 text-tiny text-muted hover:text-ink focus:border-ink focus:outline-none'
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
const navigation = useNavigation()
const projectsStore = useProjects()
const roster = useRoster()
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

type ColumnSortKey = 'default' | 'title-asc' | 'title-desc' | 'id-asc' | 'id-desc' | 'priority' | 'progress'

const SORT_OPTIONS: { value: ColumnSortKey; label: string }[] = [
  { value: 'default', label: 'Padrão' },
  { value: 'id-desc', label: 'Mais recentes' },
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
const searchQuery = ref('')
const featureFilter = ref<string>(ALL_FEATURES_VALUE)
const ownerFilter = ref<string>(ALL_FILTER_VALUE)
const tagFilter = ref<string>(ALL_FILTER_VALUE)
const typeFilter = ref<string>(ALL_FILTER_VALUE)

const activeFilterCount = computed(() => {
  let n = 0
  if (featureFilter.value !== ALL_FEATURES_VALUE) n += 1
  if (ownerFilter.value !== ALL_FILTER_VALUE) n += 1
  if (tagFilter.value !== ALL_FILTER_VALUE) n += 1
  if (typeFilter.value !== ALL_FILTER_VALUE) n += 1
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
    typeFilter.value !== ALL_FILTER_VALUE,
)

function clearFilters(): void {
  searchQuery.value = ''
  featureFilter.value = ALL_FEATURES_VALUE
  ownerFilter.value = ALL_FILTER_VALUE
  tagFilter.value = ALL_FILTER_VALUE
  typeFilter.value = ALL_FILTER_VALUE
}

const topLevelItems = computed(() => backlog.items.filter((item) => !item.parent))

const visibleItems = computed(() => {
  const query = searchQuery.value.trim().toLowerCase()
  return topLevelItems.value.filter((item) => {
    if (featureFilter.value !== ALL_FEATURES_VALUE && item.feature !== featureFilter.value) return false
    if (ownerFilter.value === UNASSIGNED_FILTER_VALUE && !isUnassigned(item)) return false
    if (ownerFilter.value !== ALL_FILTER_VALUE && ownerFilter.value !== UNASSIGNED_FILTER_VALUE && item.who !== ownerFilter.value) return false
    if (tagFilter.value !== ALL_FILTER_VALUE && !item.tags.includes(tagFilter.value)) return false
    if (typeFilter.value !== ALL_FILTER_VALUE && item.type !== typeFilter.value) return false
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

watch(
  () => projectsStore.selected?.id,
  () => {
    dismissedError.value = false
  },
)
</script>

<template>
  <section :class="PAGE_CLASSES">
    <UiPageHeader :title="TASKS_TITLE">
      <template #actions>
        <UiCreateButton :disabled="!projectsStore.selected" @press="backlog.startRequest()">
          {{ NEW_REQUEST_LABEL }}
        </UiCreateButton>
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

    <UiNotice v-if="backlog.loadError && !dismissedError" :text="LOAD_ERROR_PREFIX + backlog.loadError" @close="dismissedError = true" />
    <div :class="BOARD_AREA_CLASSES">
      <div :class="GRID_SCROLL_CLASSES">
        <div
          :key="projectsStore.selected?.id"
          role="list"
          aria-label="Colunas do backlog"
          :aria-busy="backlog.loading"
          :class="GRID_CLASSES"
        >
          <section v-for="column in columns" :key="column.name" role="listitem" :aria-label="COLUMN_LABEL[column.name]" :class="COLUMN_CLASSES">
            <div :class="COLUMN_HEADER_CLASSES">
              <div :class="COLUMN_TITLE_CLASSES">
                <b>{{ COLUMN_LABEL[column.name] }}</b>
                <span :class="COLUMN_COUNT_CLASSES">{{ column.cards.length }}</span>
                <UiHandLine :class="COLUMN_UNDERLINE_CLASSES" :style="{ color: colorVar(column.colorToken) }" />
              </div>
              <select
                v-model="columnSort[column.name]"
                :class="COLUMN_SORT_SELECT_CLASSES"
                :aria-label="`Ordenar ${COLUMN_LABEL[column.name]}`"
                :data-testid="`sort-${column.name}`"
              >
                <option v-for="opt in SORT_OPTIONS" :key="opt.value" :value="opt.value">
                  {{ opt.label }}
                </option>
              </select>
            </div>
            <span v-if="!backlog.loading && !isEmptyBoard && column.cards.length === 0" :class="EMPTY_COLUMN_CLASSES">{{ EMPTY_COLUMN_TEXT }}</span>
            <TaskCard
              v-for="item in column.cards"
              :key="item.id"
              :item="item"
              :column="column.name"
              :state="itemPlan.stateFor(item)"
              :plan-line="itemPlan.lineFor(item)"
              :proposal="itemPlan.proposalFor(item)"
              @open="onOpenCard(item.id)"
              @action="onCardAction(item.id, $event)"
            />
            <div v-if="backlog.loading" class="text-caption text-faint">{{ LOADING_TEXT }}</div>
          </section>
        </div>
      </div>
      <div v-if="isEmptyBoard" :class="EMPTY_BOARD_CLASSES">
        <UiEmpty :title="EMPTY_BOARD_TITLE" :text="EMPTY_BOARD_TEXT" />
      </div>
    </div>
  </section>
</template>
