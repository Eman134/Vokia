<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import UiCreateButton from '../../ui/UiCreateButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import UiHandLine from '../../ui/UiHandLine.vue'
import UiNotice from '../../ui/UiNotice.vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import { useProjects } from '../projects/useProjects'
import TaskCard from './TaskCard.vue'
import TaskItemModal from './TaskItemModal.vue'
import PlanDialog from './PlanDialog.vue'
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
const FILTER_ROW_CLASSES = 'flex flex-wrap items-center gap-2.5'
const COLUMN_HEADER_CLASSES = 'sticky top-0 z-10 bg-bg pb-1'
const COLUMN_TITLE_CLASSES = 'relative flex w-fit items-baseline gap-2 pb-1.5 text-card-title text-ink'
const COLUMN_COUNT_CLASSES = 'text-caption text-muted'
const COLUMN_UNDERLINE_CLASSES = 'pointer-events-none absolute inset-x-0 bottom-0 h-1.5'
const EMPTY_COLUMN_CLASSES = 'font-hand text-annotation text-faint'
const GRID_SCROLL_CLASSES = 'min-w-0 md:min-h-0 md:flex-1'
const GRID_CLASSES = 'grid min-w-0 grid-cols-1 items-start gap-2 md:h-full md:grid-cols-[repeat(5,minmax(0,1fr))] md:items-stretch'
const COLUMN_CLASSES = 'flex min-w-0 flex-col gap-2.5 md:min-h-0 md:overflow-y-auto md:pb-bar-clearance md:pr-1'
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
const projectsStore = useProjects()
const projectName = computed(() => projectsStore.selected?.name ?? '')

function columnOf(item: BacklogItem): BacklogColumn {
  return item.column ?? (item.stage ? STAGE_COLUMN[item.stage] : DEFAULT_COLUMN)
}

const dismissedError = ref(false)
const featureFilter = ref<string>(ALL_FEATURES_VALUE)
const featureOptions = computed(() => [
  { label: ALL_FEATURES_LABEL, value: ALL_FEATURES_VALUE },
  ...[...new Set(backlog.items.map((item) => item.feature).filter((feature): feature is string => Boolean(feature)))]
    .sort((a, b) => a.localeCompare(b, 'pt-BR'))
    .map((feature) => ({ label: feature, value: feature })),
])
const visibleItems = computed(() =>
  backlog.items.filter((item) => featureFilter.value === ALL_FEATURES_VALUE || item.feature === featureFilter.value),
)
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
    cards: visibleItems.value.filter((item) => !item.parent && columnOf(item) === column.name),
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
      <UiSegmented v-model="featureFilter" :options="featureOptions" label="Feature" />
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
            </div>
            <span v-if="!backlog.loading && column.cards.length === 0" :class="EMPTY_COLUMN_CLASSES">{{ EMPTY_COLUMN_TEXT }}</span>
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

    <TaskItemModal />
    <PlanDialog />
  </section>
</template>
