<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import UiNotice from '../../ui/UiNotice.vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import { useProjects } from '../projects/useProjects'
import BacklogCard from './BacklogCard.vue'
import BacklogItemModal from './BacklogItemModal.vue'
import { colorVar, TAG_COLOR, TAG_LABEL } from './backlogLabels'
import {
  ALL_FEATURES_LABEL,
  ALL_FEATURES_VALUE,
  ALL_TRIAGED_LABEL,
  BACKLOG_SUBTITLE,
  BACKLOG_TITLE,
  COLUMN_META,
  DENSITY_OPTIONS,
  EMPTY_BOARD_TEXT,
  EMPTY_BOARD_TITLE,
  EMPTY_COLUMN_TEXT,
  LOAD_ERROR_PREFIX,
  LOADING_TEXT,
  MINE_LABEL,
  NEW_REQUEST_LABEL,
  PLAN_ALL_LABEL,
  SEARCH_LABEL,
  SEARCH_PLACEHOLDER,
  TYPE_FILTER_OPTIONS,
} from './backlogPageCopy'
import { DEFAULT_COLUMN, type BacklogCardDensity, type BacklogColumn, type BacklogItem, type KanbanStage } from './backlogTypes'
import { useBacklog } from './useBacklog'

const PAGE_CLASSES = 'flex min-w-0 flex-1 flex-col gap-4 overflow-y-auto pt-7 pr-8 pb-[150px] pl-2'
const HEADER_CLASSES = 'flex flex-wrap items-end gap-4'
const TITLE_CLASSES = 'mt-1 text-[32px] font-bold tracking-[-0.03em] text-ink'
const SUBTITLE_CLASSES = 'mt-1 max-w-[70ch] text-body text-muted'
const FILTER_ROW_CLASSES = 'flex flex-wrap items-center gap-2.5'
const SEARCH_CLASSES =
  'h-9 w-55 rounded-control border border-line bg-surface px-3 text-control text-ink placeholder:text-faint disabled:opacity-40'
const MINE_CLASSES = 'h-7.5 rounded-control-sm border border-line px-3 text-caption text-muted disabled:opacity-40'
const TAG_FILTER_CLASSES = 'h-7 rounded-control-sm px-2.5 text-label font-medium disabled:opacity-40'
const GRID_SCROLL_CLASSES = 'shrink-0 overflow-x-auto'
const GRID_CLASSES = 'grid min-w-0 grid-cols-[repeat(5,minmax(210px,1fr))] items-start gap-3'
const COLUMN_CLASSES = 'flex min-w-0 flex-col gap-2.5'
const TAG_TINT_PERCENT = 11

const STAGE_COLUMN: Record<KanbanStage, BacklogColumn> = {
  ready: 'Ready',
  progress: 'In progress',
  review: 'In review',
  done: 'Done',
}
const DISPLAYED_TAGS = Object.keys(TAG_LABEL).filter((tag) => tag !== 'divida') as (keyof typeof TAG_LABEL)[]

const backlog = useBacklog()
const projectsStore = useProjects()
const projectName = computed(() => projectsStore.selected?.name ?? '')
const density = ref<BacklogCardDensity>('normal')
const typeFilter = ref<string>(TYPE_FILTER_OPTIONS[0].value)

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
  featureFilter.value === ALL_FEATURES_VALUE ? backlog.items : backlog.items.filter((item) => item.feature === featureFilter.value),
)
const isEmptyBoard = computed(() => !backlog.loading && !backlog.loadError && backlog.items.length === 0 && projectName.value !== '')
watch(featureOptions, (options) => {
  if (!options.some((option) => option.value === featureFilter.value)) featureFilter.value = ALL_FEATURES_VALUE
})

const columns = computed(() =>
  COLUMN_META.map((column) => ({
    ...column,
    cards: visibleItems.value.filter((item) => columnOf(item) === column.name),
  })),
)

const pendingCount = computed(() => backlog.items.filter((item) => item.state === 'vazio').length)
const planAllLabel = computed(() =>
  pendingCount.value ? PLAN_ALL_LABEL.replace('{count}', String(pendingCount.value)) : ALL_TRIAGED_LABEL,
)
const summary = computed(() => {
  const items = backlog.items
  const count = (predicate: (item: BacklogItem) => boolean) => items.filter(predicate).length
  return [
    `${items.length} itens`,
    `${count((item) => item.state === 'triagem')} em triagem`,
    `${pendingCount.value} não triados`,
    `${count((item) => item.state === 'proposto')} propostas esperando você`,
    `${count((item) => columnOf(item) === 'In review')} em revisão`,
  ].join(' · ')
})

function tagStyle(tag: keyof typeof TAG_LABEL) {
  const token = colorVar(TAG_COLOR[tag])
  return { color: token, backgroundColor: `color-mix(in srgb, ${token} ${TAG_TINT_PERCENT}%, transparent)` }
}

watch(
  () => projectsStore.selected?.id,
  (projectId) => {
    if (projectId === undefined) {
      backlog.clear()
      return
    }
    dismissedError.value = false
    void backlog.load(projectId)
  },
  { immediate: true },
)
</script>

<template>
  <section :class="PAGE_CLASSES">
    <div :class="HEADER_CLASSES">
      <div>
        <span v-if="projectName" class="text-caption text-muted">{{ projectName }}</span>
        <h1 :class="TITLE_CLASSES">{{ BACKLOG_TITLE }}</h1>
        <p :class="SUBTITLE_CLASSES">{{ BACKLOG_SUBTITLE }}</p>
      </div>
      <div class="ml-auto flex items-center gap-2">
        <UiSegmented v-model="density" :options="DENSITY_OPTIONS" label="Densidade" disabled />
        <UiButton variant="quiet" size="lg" class="border border-line bg-surface" disabled aria-disabled="true">
          {{ NEW_REQUEST_LABEL }}
        </UiButton>
        <UiButton variant="primary" size="lg" disabled aria-disabled="true">{{ planAllLabel }}</UiButton>
      </div>
    </div>

    <div :class="FILTER_ROW_CLASSES">
      <label for="backlog-search" class="sr-only">{{ SEARCH_LABEL }}</label>
      <input
        id="backlog-search"
        :class="SEARCH_CLASSES"
        :placeholder="SEARCH_PLACEHOLDER"
        disabled
        aria-disabled="true"
      />
      <UiSegmented v-model="featureFilter" :options="featureOptions" label="Feature" />
      <UiSegmented v-model="typeFilter" :options="TYPE_FILTER_OPTIONS" label="Tipo" disabled />
      <button type="button" :class="MINE_CLASSES" aria-pressed="false" disabled aria-disabled="true">
        {{ MINE_LABEL }}
      </button>
      <div role="group" aria-label="Etiquetas" class="flex gap-1.5">
        <button
          v-for="tag in DISPLAYED_TAGS"
          :key="tag"
          type="button"
          :class="TAG_FILTER_CLASSES"
          :style="tagStyle(tag)"
          aria-pressed="false"
          disabled
          aria-disabled="true"
        >
          {{ TAG_LABEL[tag] }}
        </button>
      </div>
      <span class="ml-auto text-caption text-muted">{{ summary }}</span>
    </div>

    <UiNotice v-if="backlog.loadError && !dismissedError" :text="LOAD_ERROR_PREFIX + backlog.loadError" @close="dismissedError = true" />
    <UiEmpty v-if="isEmptyBoard" :title="EMPTY_BOARD_TITLE" :text="EMPTY_BOARD_TEXT" />

    <div :class="GRID_SCROLL_CLASSES">
      <div role="list" aria-label="Colunas do backlog" :aria-busy="backlog.loading" :class="GRID_CLASSES">
        <section v-for="column in columns" :key="column.name" role="listitem" :aria-label="column.name" :class="COLUMN_CLASSES">
          <div class="flex items-center gap-2 text-control text-ink">
            <span class="size-2.25 rounded-full" :style="{ backgroundColor: colorVar(column.dotToken) }" />
            <b>{{ column.name }}</b>
            <span class="text-caption text-muted">{{ column.cards.length }}</span>
          </div>
          <span v-if="density === 'normal'" class="text-caption text-muted">{{ column.hint }}</span>
          <BacklogCard
            v-for="item in column.cards"
            :key="item.id"
            :item="item"
            :density="density"
            @open="backlog.open(item.id)"
          />
          <div v-if="backlog.loading" class="text-caption text-faint">{{ LOADING_TEXT }}</div>
          <div v-else-if="column.cards.length === 0" class="text-caption text-faint">{{ EMPTY_COLUMN_TEXT }}</div>
        </section>
      </div>
    </div>

    <BacklogItemModal />
  </section>
</template>
