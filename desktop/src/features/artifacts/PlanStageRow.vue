<script setup lang="ts">
import { computed } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiButton from '../../ui/UiButton.vue'
import UiProgress from '../../ui/UiProgress.vue'
import UiSelect from '../../ui/UiSelect.vue'
import type { AppPage } from '../../shell/railNav'
import { useNavigation } from '../../shell/useNavigation'
import { DEFAULT_COLUMN, MOVABLE_COLUMNS, type BacklogColumn, type BacklogItem } from '../backlog/backlogTypes'
import { useBacklog } from '../backlog/useBacklog'
import { STAGE_COLUMN_LABEL, STAGE_OPEN_LABEL } from './artifactsCopy'
import { progressOf, stageChip, stagesOf } from './planProgress'

const WRAP_CLASSES = 'wrap-break-word'
const ROW_CLASSES = 'flex min-w-0 flex-col gap-3 rounded-control-lg border border-line p-3.5'
const CRITERION_CLASSES = 'flex cursor-pointer items-start gap-2.5 text-control text-ink'
const EMPTY_PROGRESS = { done: 0, total: 0, percent: 0 }
const BACKLOG_PAGE: AppPage = 'backlog'

const props = defineProps<{ stage: BacklogItem; parentTitle?: string }>()

const backlog = useBacklog()
const navigation = useNavigation()
const column = computed<BacklogColumn>(() => props.stage.column ?? DEFAULT_COLUMN)
// Uma etapa em execução (In progress, In review) aparece com a coluna atual, que não é destino de moveTo.
const columnOptions = computed(() =>
  (MOVABLE_COLUMNS.includes(column.value) ? MOVABLE_COLUMNS : [column.value, ...MOVABLE_COLUMNS]).map((value) => ({
    value,
    label: value,
  })),
)
const criteria = computed(() => props.stage.criteria ?? [])
const chip = computed(() => stageChip(props.stage))
const children = computed(() => stagesOf(backlog.items, props.stage.id))
const childCriteria = computed(() => children.value.flatMap((child) => child.criteria ?? []))
function moveTo(target: string | undefined): void {
  const next = MOVABLE_COLUMNS.find((candidate) => candidate === target)
  if (next) backlog.moveTo(props.stage.id, next)
}

// O modal do card mora na página Backlog: abrir sem navegar não mostraria nada.
function openCard(): void {
  backlog.open(props.stage.id)
  navigation.goTo(BACKLOG_PAGE)
}

const childProgress = computed(
  () =>
    progressOf(childCriteria.value.filter((criterion) => criterion.done).length, childCriteria.value.length) ??
    EMPTY_PROGRESS,
)
</script>

<template>
  <article :class="ROW_CLASSES" :aria-label="`Etapa ${stage.title}`">
    <header class="flex min-w-0 flex-col items-start gap-2 sm:flex-row sm:gap-2.5">
      <div class="flex min-w-0 flex-1 flex-col gap-0.5">
        <p v-if="parentTitle" class="truncate text-meta text-muted">↳ {{ parentTitle }}</p>
        <span class="font-mono text-caption text-muted">{{ stage.id }}</span>
        <h4 :class="['text-control font-semibold text-ink', WRAP_CLASSES]">{{ stage.title }}</h4>
      </div>
      <UiBadge :status="chip.status" :label="chip.label" />
    </header>

    <div class="flex min-w-0 flex-wrap items-center gap-2">
      <UiSelect
        :model-value="column"
        :options="columnOptions"
        :label="STAGE_COLUMN_LABEL"
        data-testid="stage-column"
        @update:model-value="moveTo"
      />
      <UiButton variant="quiet" size="sm" data-testid="stage-open" @press="openCard">{{ STAGE_OPEN_LABEL }}</UiButton>
    </div>

    <div v-if="children.length" class="flex flex-col gap-1.5">
      <div class="flex flex-wrap justify-between gap-x-3 text-meta text-muted">
        <span>{{ childProgress.done }} de {{ childProgress.total }} critérios dos filhos marcados</span>
        <span>{{ childProgress.percent }}%</span>
      </div>
      <UiProgress :value="childProgress.percent" />
    </div>

    <div v-if="criteria.length" class="flex flex-col gap-2">
      <label v-for="criterion in criteria" :key="criterion.id" :class="CRITERION_CLASSES">
        <input
          type="checkbox"
          :checked="criterion.done"
          class="mt-1 size-4 flex-none cursor-pointer"
          @change="backlog.toggleCriterion(stage.id, criterion.id)"
        />
        <span :class="['min-w-0 flex-1', WRAP_CLASSES, criterion.done && 'text-muted line-through']">{{ criterion.text }}</span>
      </label>
    </div>
  </article>
</template>
