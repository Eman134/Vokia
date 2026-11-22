<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import UiCreateButton from '../../ui/UiCreateButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import UiHandLine from '../../ui/UiHandLine.vue'
import { HAND_OUTLINE_CLASSES, INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import UiMarker from '../../ui/UiMarker.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import { useProjects } from '../projects/useProjects'
import BacklogCard from './BacklogCard.vue'
import BacklogItemModal from './BacklogItemModal.vue'
import PlanDialog from './PlanDialog.vue'
import { colorVar } from './backlogLabels'
import { tagLabel, tagMarkerTone } from './tags'
import {
  ALL_FEATURES_LABEL,
  ALL_FEATURES_VALUE,
  BACKLOG_TITLE,
  COLUMN_META,
  EMPTY_BOARD_TEXT,
  EMPTY_BOARD_TITLE,
  EMPTY_COLUMN_TEXT,
  LOAD_ERROR_PREFIX,
  LOADING_TEXT,
  NEW_REQUEST_LABEL,
} from './backlogPageCopy'
import {
  DEFAULT_COLUMN,
  type BacklogCardAction,
  type BacklogColumn,
  type BacklogItem,
  type KanbanStage,
} from './backlogTypes'
import { useBacklog } from './useBacklog'
import { useItemPlan, type PlanTab } from './useItemPlan'
import { useRunItem } from './useRunItem'

const PAGE_CLASSES = `flex min-w-0 flex-1 flex-col gap-4 overflow-y-auto ${PAGE_INSET_CLASSES}`
const FILTER_ROW_CLASSES = 'flex flex-wrap items-center gap-2.5'
const TAG_FILTER_BASE_CLASSES = `cursor-pointer border-[1.5px] px-1 py-0.5 text-label font-medium ${INK_FOCUS_CLASSES}`
const COLUMN_TITLE_CLASSES = 'relative flex w-fit items-baseline gap-2 pb-1.5 text-card-title text-ink'
const COLUMN_COUNT_CLASSES = 'text-caption text-muted'
const COLUMN_UNDERLINE_CLASSES = 'pointer-events-none absolute inset-x-0 bottom-0 h-1.5'
const EMPTY_COLUMN_CLASSES = 'font-hand text-annotation text-faint'
const GRID_SCROLL_CLASSES = 'min-w-0'
const GRID_CLASSES = 'grid min-w-0 grid-cols-1 items-start gap-2 md:grid-cols-[repeat(5,minmax(0,1fr))]'
const COLUMN_CLASSES = 'flex min-w-0 flex-col gap-2.5'
const BOARD_AREA_CLASSES = 'flex flex-1 flex-col'
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
const projectsStore = useProjects()
const projectName = computed(() => projectsStore.selected?.name ?? '')

function columnOf(item: BacklogItem): BacklogColumn {
  return item.column ?? (item.stage ? STAGE_COLUMN[item.stage] : DEFAULT_COLUMN)
}

const dismissedError = ref(false)
const featureFilter = ref<string>(ALL_FEATURES_VALUE)
const activeTag = ref<string | null>(null)
const availableTags = computed(() =>
  [...new Set(backlog.items.flatMap((item) => item.tags))].sort((a, b) => a.localeCompare(b, 'pt-BR')),
)
const featureOptions = computed(() => [
  { label: ALL_FEATURES_LABEL, value: ALL_FEATURES_VALUE },
  ...[...new Set(backlog.items.map((item) => item.feature).filter((feature): feature is string => Boolean(feature)))]
    .sort((a, b) => a.localeCompare(b, 'pt-BR'))
    .map((feature) => ({ label: feature, value: feature })),
])
const visibleItems = computed(() =>
  backlog.items.filter(
    (item) =>
      (featureFilter.value === ALL_FEATURES_VALUE || item.feature === featureFilter.value) &&
      (activeTag.value === null || item.tags.includes(activeTag.value)),
  ),
)
const isEmptyBoard = computed(() => !backlog.loading && !backlog.loadError && backlog.items.length === 0 && projectName.value !== '')
watch(featureOptions, (options) => {
  if (!options.some((option) => option.value === featureFilter.value)) featureFilter.value = ALL_FEATURES_VALUE
})
watch(availableTags, (tags) => {
  if (activeTag.value !== null && !tags.includes(activeTag.value)) activeTag.value = null
})

const PLAN_TAB_BY_ACTION: Record<Exclude<BacklogCardAction, 'iniciar'>, PlanTab> = { triar: 'pedir', revisar: 'pedir', vincular: 'vincular' }

function onCardAction(itemId: string, action: BacklogCardAction): void {
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
    cards: visibleItems.value.filter((item) => !item.parent && columnOf(item) === column.name),
  })),
)

watch(
  () => projectsStore.selected?.id,
  () => {
    activeTag.value = null
    dismissedError.value = false
  },
)
</script>

<template>
  <section :class="PAGE_CLASSES">
    <UiPageHeader :title="BACKLOG_TITLE" :project-name="projectName">
      <template #actions>
        <UiCreateButton :disabled="!projectsStore.selected" @press="backlog.startRequest()">
          {{ NEW_REQUEST_LABEL }}
        </UiCreateButton>
      </template>
    </UiPageHeader>

    <div :class="FILTER_ROW_CLASSES">
      <UiSegmented v-model="featureFilter" :options="featureOptions" label="Feature" />
      <div v-if="availableTags.length" role="group" aria-label="Etiquetas" class="flex flex-wrap gap-1.5">
        <button
          v-for="tag in availableTags"
          :key="tag"
          type="button"
          :class="[TAG_FILTER_BASE_CLASSES, activeTag === tag ? HAND_OUTLINE_CLASSES : 'border-transparent']"
          :aria-pressed="activeTag === tag"
          @click="activeTag = activeTag === tag ? null : tag"
        >
          <UiMarker :tone="tagMarkerTone(tag)">{{ tagLabel(tag) }}</UiMarker>
        </button>
      </div>
    </div>

    <UiNotice v-if="backlog.loadError && !dismissedError" :text="LOAD_ERROR_PREFIX + backlog.loadError" @close="dismissedError = true" />
    <div :class="BOARD_AREA_CLASSES">
      <div :class="GRID_SCROLL_CLASSES">
        <div role="list" aria-label="Colunas do backlog" :aria-busy="backlog.loading" :class="GRID_CLASSES">
          <section v-for="column in columns" :key="column.name" role="listitem" :aria-label="column.name" :class="COLUMN_CLASSES">
            <div :class="COLUMN_TITLE_CLASSES">
              <b>{{ column.name }}</b>
              <span :class="COLUMN_COUNT_CLASSES">{{ column.cards.length }}</span>
              <UiHandLine :class="COLUMN_UNDERLINE_CLASSES" :style="{ color: colorVar(column.colorToken) }" />
            </div>
            <span v-if="!backlog.loading && column.cards.length === 0" :class="EMPTY_COLUMN_CLASSES">{{ EMPTY_COLUMN_TEXT }}</span>
            <BacklogCard
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

    <BacklogItemModal />
    <PlanDialog />
  </section>
</template>
