<script setup lang="ts">
import { computed, ref, useId, watch } from 'vue'
import type { TurnPlan, TurnPlanStep, TurnPlanStepStatus } from './events'
import type { TurnPlanCardStatus } from './useTurnPlans'
import { INK_FOCUS_CLASSES } from '../../ui/handDrawn'

const props = defineProps<{
  plan: TurnPlan
  status: TurnPlanCardStatus
  compact?: boolean
  title?: string
  defaultOpen?: boolean
}>()

const CARD_TITLE = 'Plano do turno'
const REPLACED_TITLE = 'Plano anterior'
const STATUS_LABELS: Record<TurnPlanCardStatus, string> = {
  active: 'Em andamento',
  completed: 'Concluído',
  partial: 'Parcial',
  replaced: 'Substituído',
}
const STEP_STATUS_SR: Record<TurnPlanStepStatus, string> = {
  pending: 'Pendente',
  in_progress: 'Em andamento',
  completed: 'Concluída',
}

const CARD_CLASSES = 'flex min-w-0 flex-col rounded-md border border-line bg-surface'
const CARD_REPLACED_CLASSES = 'flex min-w-0 flex-col rounded-md border border-line/70 bg-surface-2/50'
const HEADER_BUTTON_CLASSES = `flex w-full cursor-pointer items-center justify-between gap-2 text-left transition-colors hover:bg-hover ${INK_FOCUS_CLASSES}`
const BADGE_BASE_CLASSES = 'shrink-0 rounded-sm px-1.5 py-0.5 text-tiny font-medium'
const BADGE_CLASSES: Record<TurnPlanCardStatus, string> = {
  active: `${BADGE_BASE_CLASSES} bg-accent-soft text-accent`,
  completed: `${BADGE_BASE_CLASSES} bg-ok-soft text-ok`,
  partial: `${BADGE_BASE_CLASSES} bg-warn-soft text-warn-text`,
  replaced: `${BADGE_BASE_CLASSES} bg-surface-2 text-muted`,
}
const STEP_ROW_BASE_CLASSES = 'flex min-w-0 items-start gap-2 rounded-sm px-1.5 py-1 text-caption'
const STEP_ROW_ACTIVE_CLASSES = `${STEP_ROW_BASE_CLASSES} bg-accent-soft text-ink font-medium`
const STEP_ROW_COMPLETED_CLASSES = `${STEP_ROW_BASE_CLASSES} text-muted`
const STEP_ROW_PENDING_CLASSES = `${STEP_ROW_BASE_CLASSES} text-muted`

const open = ref(props.defaultOpen ?? props.status !== 'replaced')
const listId = useId()

watch(
  () => props.status,
  (next) => {
    if (next === 'replaced') {
      open.value = false
    }
  },
)

const totalCount = computed(() => props.plan.steps.length)
const completedCount = computed(
  () => props.plan.steps.filter((step) => step.status === 'completed').length,
)
const progressPercent = computed(() =>
  totalCount.value > 0 ? Math.round((completedCount.value / totalCount.value) * 100) : 0,
)
const progressLabel = computed(
  () => `${completedCount.value}/${totalCount.value} ${completedCount.value === 1 ? 'concluída' : 'concluídas'}`,
)
const titleText = computed(() => (props.status === 'replaced' ? REPLACED_TITLE : (props.title ?? CARD_TITLE)))
const progressBarFillClasses = computed(() =>
  props.status === 'completed' ? 'bg-ok' : 'bg-accent',
)

function stepDisplayText(step: TurnPlanStep): string {
  if (step.status === 'in_progress' && step.activeText && step.activeText.trim() !== '') {
    return step.activeText
  }
  return step.text
}

function stepRowClasses(status: TurnPlanStepStatus): string {
  if (status === 'in_progress') return STEP_ROW_ACTIVE_CLASSES
  if (status === 'completed') return STEP_ROW_COMPLETED_CLASSES
  return STEP_ROW_PENDING_CLASSES
}
</script>

<template>
  <section
    v-if="plan.steps.length > 0"
    :class="[status === 'replaced' ? CARD_REPLACED_CLASSES : CARD_CLASSES, compact ? 'max-w-full' : 'max-w-xl']"
    :data-plan-status="status"
    data-testid="turn-plan-card"
  >
    <button
      type="button"
      :class="[HEADER_BUTTON_CLASSES, compact ? 'px-2.5 py-1.5 rounded-md' : 'px-3 py-2 rounded-md']"
      :aria-expanded="open"
      :aria-controls="listId"
      data-testid="turn-plan-toggle"
      @click="open = !open"
    >
      <span class="flex min-w-0 items-center gap-2">
        <svg
          class="size-3.5 shrink-0 text-muted"
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path d="M3 4.5h10" />
          <path d="M3 8h7" />
          <path d="M3 11.5h5" />
          <path d="m10.5 11 1.2 1.2 2.3-2.4" />
        </svg>
        <span class="truncate text-caption font-semibold text-ink">{{ titleText }}</span>
        <span :class="BADGE_CLASSES[status]">{{ STATUS_LABELS[status] }}</span>
      </span>
      <span class="flex shrink-0 items-center gap-2 text-small text-muted tabular-nums">
        <span data-testid="turn-plan-progress">{{ progressLabel }}</span>
        <span aria-hidden="true">{{ open ? '▾' : '▸' }}</span>
      </span>
    </button>

    <div v-if="open" :id="listId" :class="['flex flex-col gap-2 border-t border-line', compact ? 'px-2.5 py-2' : 'px-3 py-2.5']">
      <div class="h-1 w-full overflow-hidden rounded-full bg-surface-2" aria-hidden="true">
        <div
          class="h-full rounded-full transition-[width] duration-200 motion-reduce:transition-none"
          :class="progressBarFillClasses"
          :style="{ width: `${progressPercent}%` }"
        />
      </div>
      <p v-if="plan.explanation" class="wrap-break-word text-small text-muted">
        {{ plan.explanation }}
      </p>
      <ol class="flex flex-col gap-0.5" data-testid="turn-plan-steps">
        <li
          v-for="step in plan.steps"
          :key="step.id"
          :class="stepRowClasses(step.status)"
          :data-step-status="step.status"
        >
          <!-- Formas distintas além da cor: concluída = círculo preenchido com ✓; em andamento = anel duplo com centro ativo; pendente = círculo vazado tracejado. -->
          <span class="mt-0.5 flex size-4 shrink-0 items-center justify-center" :aria-label="STEP_STATUS_SR[step.status]">
            <svg
              v-if="step.status === 'completed'"
              class="size-3.5 text-ok"
              viewBox="0 0 16 16"
              fill="none"
              aria-hidden="true"
            >
              <circle cx="8" cy="8" r="6.5" fill="currentColor" />
              <path
                d="M5.2 8.2 7.1 10.1 11 6"
                stroke="var(--color-accent-ink)"
                stroke-width="1.6"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
            <svg
              v-else-if="step.status === 'in_progress'"
              class="size-3.5 text-accent"
              viewBox="0 0 16 16"
              fill="none"
              aria-hidden="true"
            >
              <circle cx="8" cy="8" r="6" stroke="currentColor" stroke-width="1.5" />
              <circle cx="8" cy="8" r="2.7" fill="currentColor" class="animate-pulse motion-reduce:animate-none" />
            </svg>
            <svg
              v-else
              class="size-3.5 text-faint"
              viewBox="0 0 16 16"
              fill="none"
              aria-hidden="true"
            >
              <circle
                cx="8"
                cy="8"
                r="5.5"
                stroke="currentColor"
                stroke-width="1.4"
                stroke-dasharray="2.4 2"
              />
            </svg>
          </span>
          <span
            class="wrap-break-word min-w-0 flex-1"
            :class="step.status === 'completed' ? 'line-through decoration-line' : ''"
          >
            {{ stepDisplayText(step) }}
          </span>
        </li>
      </ol>
    </div>
  </section>
</template>
