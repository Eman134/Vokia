<script setup lang="ts">
import { computed } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiProgress from '../../ui/UiProgress.vue'
import { STATE_BADGE, STATE_CHIP } from '../backlog/backlogLabels'
import type { BacklogItem, BacklogState } from '../backlog/backlogTypes'
import { useBacklog } from '../backlog/useBacklog'
import type { ParsedProposal } from '../backlog/planProposal'
import PlanStageRow from './PlanStageRow.vue'
import { planProgress } from './planProgress'

const WRAP_CLASSES = 'wrap-break-word'
const SECTION_CLASSES = 'flex min-w-0 flex-col gap-3'
const SECTION_TITLE_CLASSES = 'text-small font-semibold text-muted'
const STAGE_MARK_CLASSES = 'grid size-5.5 flex-none place-items-center rounded-full border-[1.5px] border-line text-tiny font-bold text-muted'
const CRITERION_CLASSES = 'flex cursor-pointer items-start gap-2.5 text-control text-ink'
const TASK_SEPARATOR = ';'

const props = defineProps<{
  item: BacklogItem
  proposal: ParsedProposal
  state: BacklogState
  stages: BacklogItem[]
}>()

const backlog = useBacklog()

const progress = computed(() => planProgress(props.stages))

function tasksOf(lot: ParsedProposal['lots'][number]): string[] {
  return lot.tasks
    .split(TASK_SEPARATOR)
    .map((task) => task.trim())
    .filter((task) => task !== '')
}
</script>

<template>
  <article class="flex min-w-0 flex-col gap-6" :aria-label="`Plano de ${item.title}`">
    <header class="flex min-w-0 flex-col gap-2">
      <p class="text-caption text-muted">Artefatos › Plano · <span class="font-mono">{{ item.id }}</span></p>
      <div class="flex flex-col items-start gap-2 sm:flex-row sm:items-center sm:gap-2.5">
        <h3 :class="['min-w-0 flex-1 text-lead text-ink', WRAP_CLASSES]">{{ item.title }}</h3>
        <UiBadge :status="STATE_BADGE[state]" :label="STATE_CHIP[state]" />
      </div>
      <p v-if="proposal.estimate" class="text-small text-muted">Estimativa: {{ proposal.estimate }}</p>
      <p v-if="proposal.summary" :class="['whitespace-pre-wrap text-body text-ink', WRAP_CLASSES]">{{ proposal.summary }}</p>
      <div v-if="progress" class="flex flex-col gap-1.5">
        <div class="flex flex-wrap justify-between gap-x-3 text-meta text-muted">
          <span>{{ progress.done }} de {{ progress.total }} etapas feitas</span>
          <span>{{ progress.percent }}%</span>
        </div>
        <UiProgress :value="progress.percent" />
      </div>
    </header>

    <section aria-label="Etapas" :class="SECTION_CLASSES">
      <h4 :class="SECTION_TITLE_CLASSES">Etapas</h4>
      <div v-if="stages.length" class="flex flex-col gap-3">
        <PlanStageRow v-for="stage in stages" :key="stage.id" :stage="stage" :parent-title="item.title" />
      </div>
      <ol v-else class="flex flex-col gap-3">
        <li v-for="lot in proposal.lots" :key="lot.n" class="flex min-w-0 flex-col gap-2 rounded-control-lg border border-line p-3.5">
          <p class="truncate text-meta text-muted">↳ {{ item.title }}</p>
          <div class="flex min-w-0 items-start gap-2.5">
            <span :class="STAGE_MARK_CLASSES">{{ lot.n }}</span>
            <div class="flex min-w-0 flex-1 flex-col gap-0.5">
              <span :class="['text-control font-semibold text-ink', WRAP_CLASSES]">{{ lot.title }}</span>
              <span :class="['text-caption text-muted', WRAP_CLASSES]">{{ lot.meta }}</span>
            </div>
          </div>
          <ul v-if="tasksOf(lot).length" class="flex flex-col gap-1 pl-7">
            <li v-for="task in tasksOf(lot)" :key="task" :class="['flex items-start gap-2 text-small text-muted', WRAP_CLASSES]">
              <span class="text-faint" aria-hidden="true">○</span><span class="min-w-0 flex-1">{{ task }}</span>
            </li>
          </ul>
        </li>
      </ol>
    </section>

    <section v-if="item.criteria?.length" aria-label="Critérios" :class="SECTION_CLASSES">
      <h4 :class="SECTION_TITLE_CLASSES">Critérios</h4>
      <ul class="flex flex-col gap-2">
        <li v-for="criterion in item.criteria" :key="criterion.id">
          <label :class="CRITERION_CLASSES">
            <input
              type="checkbox"
              :checked="criterion.done"
              class="mt-1 size-4 flex-none"
              @change="backlog.toggleCriterion(item.id, criterion.id)"
            />
            <span :class="['min-w-0 flex-1', WRAP_CLASSES, criterion.done && 'text-muted line-through']">{{ criterion.text }}</span>
          </label>
        </li>
      </ul>
    </section>

    <section v-if="proposal.risk" aria-label="Riscos" :class="SECTION_CLASSES">
      <h4 :class="SECTION_TITLE_CLASSES">Riscos</h4>
      <p :class="['text-small text-ink', WRAP_CLASSES]">{{ proposal.risk }}</p>
    </section>
  </article>
</template>
