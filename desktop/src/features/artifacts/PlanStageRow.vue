<script setup lang="ts">
import { computed } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiButton from '../../ui/UiButton.vue'
import UiCheckbox from '../../ui/UiCheckbox.vue'
import UiHandLine from '../../ui/UiHandLine.vue'
import UiIcon from '../../ui/UiIcon.vue'
import UiProgress from '../../ui/UiProgress.vue'
import UiSelect from '../../ui/UiSelect.vue'
import { DEFAULT_JITTER, sketchCheck } from '../../ui/sketch/sketchPath'
import type { AppPage } from '../../shell/railNav'
import { useNavigation } from '../../shell/useNavigation'
import { DEFAULT_COLUMN, MOVABLE_COLUMNS, type BacklogColumn, type BacklogItem } from '../backlog/backlogTypes'

const DONE_COLUMN: BacklogColumn = 'Done'
const CHECK_BOX = 16
const CHECK_SEED = 5
const CHECK_PATH = sketchCheck(CHECK_BOX, CHECK_BOX, CHECK_SEED, DEFAULT_JITTER)
const STRIKE_CLASSES = 'pointer-events-none absolute inset-x-0 top-1/2 h-2 -translate-y-1/2 text-muted'
import { useBacklog } from '../backlog/useBacklog'
import { STAGE_COLUMN_LABEL, STAGE_OPEN_LABEL } from './artifactsCopy'
import { progressOf, stageChip, stagesOf } from './planProgress'

const WRAP_CLASSES = 'wrap-break-word'
const ROW_CLASSES = 'flex min-w-0 flex-col gap-3 rounded-md border border-line p-3.5'
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
const isDone = computed(() => column.value === DONE_COLUMN)
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
        <div class="flex min-w-0 items-start gap-1.5">
          <UiIcon v-if="isDone" :path="CHECK_PATH" :size="CHECK_BOX" class="mt-0.5 flex-none text-ink-pen" />
          <h4 :class="['relative min-w-0 text-control font-semibold', WRAP_CLASSES, isDone ? 'text-muted' : 'text-ink']">
            {{ stage.title }}
            <UiHandLine v-if="isDone" variant="strike" :class="STRIKE_CLASSES" />
          </h4>
        </div>
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
      <UiProgress :value="childProgress.percent" pencil />
    </div>

    <div v-if="criteria.length" class="flex flex-col gap-2">
      <UiCheckbox v-for="criterion in criteria" :key="criterion.id" :checked="criterion.done" @change="backlog.toggleCriterion(stage.id, criterion.id)">
        {{ criterion.text }}
      </UiCheckbox>
    </div>
  </article>
</template>
