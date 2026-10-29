<script setup lang="ts">
import { computed } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import UiCommandBlock from '../../ui/UiCommandBlock.vue'
import {
  ITEM_STATE_BADGE,
  ITEM_STATE_LABEL,
  PLAN_STAGES,
  type ItemState,
  type SamplePlan,
} from './planosSample'

const DETAIL_CLASSES = 'flex min-w-0 flex-[999_1_560px] flex-wrap items-start gap-4'
const STAGES_CLASSES = 'flex-[1_1_220px] max-w-[260px] rounded-dialog border border-line bg-surface p-2.5'
const STAGE_LIST_CLASSES = 'flex flex-col gap-0.5'
const STAGE_ROW_CLASSES = 'flex items-center gap-3 rounded-card p-2.5'
const STAGE_ROW_STATE: Record<ItemState, string> = { done: '', now: 'bg-surface-2', next: 'text-muted' }
const STAGE_MARK_CLASSES =
  'grid size-6.5 flex-none place-items-center rounded-full text-small font-bold'
const STAGE_MARK_STATE: Record<ItemState, string> = {
  done: 'bg-accent-soft text-accent',
  now: 'bg-accent text-accent-ink',
  next: 'border-[1.5px] border-line text-muted',
}
const STAGE_SUB_STATE: Record<ItemState, string> = { done: 'text-muted', now: 'text-accent', next: 'text-muted' }

const LOTS_CLASSES = 'flex-[999_1_380px] min-w-0 flex flex-col gap-2.5'
const LOT_GROUP_CLASSES = 'flex flex-col gap-2.5'
const LOT_CLASSES = 'flex flex-wrap items-center gap-2.5 bg-surface px-4 py-3.5'
const LOT_ROUNDED_ALONE = 'rounded-panel'
const LOT_ROUNDED_WITH_STEPS = 'rounded-t-panel rounded-b-chip'
const LOT_STATE: Record<ItemState, string> = {
  done: 'border border-line',
  now: 'border-[1.5px] border-accent',
  next: 'border border-line text-muted',
}
const LOT_MARK_CLASSES = 'grid size-5.5 flex-none place-items-center rounded-full text-tiny font-bold'
const LOT_MARK_STATE: Record<ItemState, string> = {
  done: 'bg-accent-soft text-accent',
  now: 'border-2 border-accent text-accent',
  next: 'border-[1.5px] border-line',
}
const STEP_CLASSES = 'ml-3 flex items-center gap-3 rounded-control-lg px-3.5 py-2.5 text-control'
const STEP_STATE: Record<ItemState, string> = { done: '', now: 'bg-accent-soft', next: 'text-muted' }
const STEP_ICON: Record<ItemState, string> = { done: '✓', now: '●', next: '○' }
const SUB_PLAN_CLASSES =
  'mt-0.5 mb-1.5 ml-7 flex w-[calc(100%_-_28px)] cursor-default items-center gap-2.5 rounded-control-lg border border-dashed border-accent bg-accent-soft px-3 py-2 text-left text-caption text-ink'
const CRITERIA_CLASSES = 'flex flex-col gap-2 rounded-panel border border-line bg-surface p-4'
const COMMANDS_CLASSES = 'flex flex-col gap-1'

const DONE_ICON = '✓'

const props = defineProps<{ plan: SamplePlan }>()

function stateAt(position: number, current: number): ItemState {
  if (position < current) return 'done'
  return position === current ? 'now' : 'next'
}

const stages = computed(() =>
  PLAN_STAGES.map((stage, index) => {
    const state = stateAt(index + 1, props.plan.stage)
    return {
      name: stage.name,
      sub: state === 'done' ? ITEM_STATE_LABEL.done : state === 'now' ? stage.sub : '',
      state,
      icon: state === 'done' ? DONE_ICON : String(index + 1),
    }
  }),
)
</script>

<template>
  <div :class="DETAIL_CLASSES">
    <section aria-label="Etapas" :class="STAGES_CLASSES">
      <ol :class="STAGE_LIST_CLASSES">
        <li
          v-for="stage in stages"
          :key="stage.name"
          :class="[STAGE_ROW_CLASSES, STAGE_ROW_STATE[stage.state]]"
        >
          <span :class="[STAGE_MARK_CLASSES, STAGE_MARK_STATE[stage.state]]">{{ stage.icon }}</span>
          <div>
            <b class="text-control font-semibold">{{ stage.name }}</b>
            <div :class="['text-small', STAGE_SUB_STATE[stage.state]]">{{ stage.sub }}</div>
          </div>
        </li>
      </ol>
    </section>

    <div :class="LOTS_CLASSES">
      <div v-for="lot in plan.lots" :key="lot.name" :class="LOT_GROUP_CLASSES">
        <div
          :class="[
            LOT_CLASSES,
            lot.steps.length ? LOT_ROUNDED_WITH_STEPS : LOT_ROUNDED_ALONE,
            LOT_STATE[lot.state],
          ]"
        >
          <span :class="[LOT_MARK_CLASSES, LOT_MARK_STATE[lot.state]]">
            {{ lot.state === 'done' ? DONE_ICON : '' }}
          </span>
          <b class="text-body font-semibold">{{ lot.name }}</b>
          <span class="text-caption text-muted">{{ lot.meta }}</span>
          <span class="ml-auto text-small text-muted">{{ lot.note }}</span>
          <UiBadge :status="ITEM_STATE_BADGE[lot.state]" :label="ITEM_STATE_LABEL[lot.state]" />
        </div>

        <div v-for="step in lot.steps" :key="step.name" :class="[STEP_CLASSES, STEP_STATE[step.state]]">
          <span :class="['w-4 font-bold', step.state === 'next' ? 'text-faint' : 'text-accent']">
            {{ STEP_ICON[step.state] }}
          </span>
          <span class="min-w-0 flex-1">{{ step.name }}</span>
          <span class="font-mono text-tiny text-muted">{{ step.tag }}</span>
          <span
            title="Modelo da micro-tarefa"
            class="rounded-chip border border-line px-1.25 py-px font-mono text-[10.5px] whitespace-nowrap text-muted"
          >
            {{ step.model }}
          </span>
          <span class="w-10 text-right text-small text-muted">{{ step.who }}</span>
          <UiBadge :status="ITEM_STATE_BADGE[step.state]" :label="ITEM_STATE_LABEL[step.state]" />
        </div>

        <button v-if="lot.subPlanName" type="button" disabled aria-disabled="true" :class="SUB_PLAN_CLASSES">
          <span class="text-accent">↳</span>
          <span class="flex-1">Sub-plano da task: <b class="font-semibold">{{ lot.subPlanName }}</b></span>
          <span class="font-semibold text-accent">abrir</span>
        </button>
      </div>

      <section :class="CRITERIA_CLASSES">
        <b class="text-body font-semibold">Critérios de verificação</b>
        <div :class="COMMANDS_CLASSES">
          <UiCommandBlock v-for="command in plan.checks" :key="command" :command="command" />
        </div>
      </section>
    </div>
  </div>
</template>
