<script setup lang="ts">
import { computed, ref } from 'vue'
import UiBadge from '../../ui/UiBadge.vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import { MICRO_STATUS_BADGE } from './backlogLabels'
import { MICRO_EXPLANATION, MICRO_SUMMARY } from './backlogTypes'
import MicroTaskScript, { isScriptValid } from './MicroTaskScript.vue'
import { modelTierLabel, type PlanBreakdown } from './planBreakdown'

const props = defineProps<{
  breakdown: PlanBreakdown
  leader: string
}>()

const emit = defineEmits<{
  'open-stage': [stageId: string]
}>()

const expanded = defineModel<Set<string>>('expanded', { required: true })
const listRoot = ref<HTMLElement>()

const MICRO_PORTRAIT_SIZE = 20
const HEADER_TITLE_CLASSES = 'text-label font-semibold uppercase tracking-wider text-muted'
const ROW_CARD_CLASSES = 'flex flex-col gap-2 rounded-control-lg border border-line p-3'
const ROW_BUTTON_CLASSES =
  'flex min-w-0 flex-1 cursor-pointer flex-wrap items-center gap-2 text-left text-control text-ink hover:opacity-80 focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-accent'
const BADGES_WRAP_CLASSES = 'ml-auto flex flex-wrap items-center gap-2 max-md:w-full max-md:pl-5'
const FALLBACK_PANEL_CLASSES = 'flex flex-col gap-2 rounded-control-lg bg-surface-2 p-3 text-caption'

const doneCount = computed(() => props.breakdown.steps.filter((step) => step.status === 'feito').length)
const hasAnyValidScript = computed(() =>
  props.breakdown.steps.some((step) => step.script !== undefined && isScriptValid(step.script)),
)

function panelId(stepId: string): string {
  return `breakdown-step-${stepId}`
}

function toggle(stepId: string): void {
  const next = new Set(expanded.value)
  if (!next.delete(stepId)) next.add(stepId)
  expanded.value = next
}

function onRowKeydown(event: KeyboardEvent): void {
  if (event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLInputElement) return
  if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp' && event.key !== 'Home' && event.key !== 'End') return
  const buttons = Array.from(listRoot.value?.querySelectorAll<HTMLButtonElement>('[data-breakdown-row]') ?? [])
  if (buttons.length === 0) return
  const currentIndex = buttons.indexOf(event.currentTarget as HTMLButtonElement)
  if (currentIndex === -1) return

  event.preventDefault()
  let nextIndex = currentIndex
  if (event.key === 'ArrowDown') nextIndex = (currentIndex + 1) % buttons.length
  else if (event.key === 'ArrowUp') nextIndex = (currentIndex - 1 + buttons.length) % buttons.length
  else if (event.key === 'Home') nextIndex = 0
  else if (event.key === 'End') nextIndex = buttons.length - 1
  buttons[nextIndex]?.focus()
}
</script>

<template>
  <section ref="listRoot" class="flex flex-col gap-2" data-testid="plan-breakdown">
    <div class="flex items-baseline justify-between gap-2">
      <h3 :class="HEADER_TITLE_CLASSES">Desdobramento</h3>
      <span class="text-caption text-muted">{{ MICRO_SUMMARY(breakdown.steps.length, doneCount) }}</span>
    </div>

    <div v-for="step in breakdown.steps" :key="step.id" :class="ROW_CARD_CLASSES">
      <div class="flex w-full flex-wrap items-center gap-2">
        <button
          type="button"
          data-breakdown-row
          class="flex shrink-0 cursor-pointer items-center text-control text-ink hover:opacity-80 focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-accent"
          :aria-expanded="expanded.has(step.id)"
          :aria-controls="panelId(step.id)"
          :aria-label="`${step.id} ${step.title}`"
          @click="toggle(step.id)"
          @keydown="onRowKeydown"
        >
          <span aria-hidden="true">{{ expanded.has(step.id) ? '▾' : '▸' }}</span>
        </button>

        <button
          v-if="step.stageId"
          type="button"
          data-testid="breakdown-open-stage"
          class="shrink-0 cursor-pointer font-mono text-meta text-accent hover:underline focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-accent"
          :title="`Abrir ${step.id}`"
          @click="emit('open-stage', step.stageId)"
        >
          {{ step.id }}
        </button>
        <span v-else class="shrink-0 font-mono text-meta text-muted">{{ step.id }}</span>

        <button
          type="button"
          :class="ROW_BUTTON_CLASSES"
          @click="toggle(step.id)"
        >
          <span class="min-w-0 flex-1 wrap-break-word font-medium">{{ step.title }}</span>
          <span :class="BADGES_WRAP_CLASSES">
            <AgentPortrait v-if="step.who" :who="step.who" :size="MICRO_PORTRAIT_SIZE" />
            <UiBadge v-if="modelTierLabel(step.model)" status="backlog" :label="modelTierLabel(step.model)!" />
            <UiBadge :status="MICRO_STATUS_BADGE[step.status]" :label="step.status" />
          </span>
        </button>
      </div>

      <div v-show="expanded.has(step.id)" :id="panelId(step.id)">
        <MicroTaskScript
          v-if="step.script && isScriptValid(step.script)"
          :script="step.script"
          :model="modelTierLabel(step.model) ?? step.model ?? ''"
          :leader="leader"
        />
        <div v-else :class="FALLBACK_PANEL_CLASSES">
          <div v-if="step.tasks.length" class="flex flex-col gap-1">
            <h4 class="font-semibold text-ink">Tarefas</h4>
            <ul class="list-disc pl-4 text-muted">
              <li v-for="task in step.tasks" :key="task">{{ task }}</li>
            </ul>
          </div>
          <div v-if="step.goal" class="flex flex-col gap-1">
            <h4 class="font-semibold text-ink">Pronto quando</h4>
            <p class="text-muted">{{ step.goal }}</p>
          </div>
        </div>
      </div>
    </div>

    <p v-if="hasAnyValidScript" class="text-caption text-muted">{{ MICRO_EXPLANATION(leader) }}</p>
  </section>
</template>
