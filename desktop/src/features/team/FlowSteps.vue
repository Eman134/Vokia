<script setup lang="ts">
import { computed } from 'vue'
import { useFlow } from './useFlow'
import { FLOW_FINISH_LABEL, FLOW_LAST_STEP_TITLE, FLOW_READY_TEXT } from './flowSteps'
import UiButton from '../../ui/UiButton.vue'

type StepState = 'feita' | 'atual' | 'depois'

const STEP_CLASSES: Record<StepState, string> = {
  feita: 'text-ok',
  atual: 'bg-surface-2 font-semibold text-ink',
  depois: 'text-muted',
}

const flow = useFlow()
const isLast = computed(() => flow.step === flow.total)
const title = computed(() =>
  isLast.value
    ? `${FLOW_LAST_STEP_TITLE} · ${FLOW_READY_TEXT} · ${flow.step}/${flow.total}`
    : `Etapa ${flow.step} de ${flow.total} · ${flow.names[flow.step - 1]}`,
)

function stateOf(index: number): StepState {
  const number = index + 1
  if (number < flow.step) {
    return 'feita'
  }
  return number === flow.step ? 'atual' : 'depois'
}
</script>

<template>
  <div class="flex min-w-0 items-center gap-3">
    <span class="shrink-0 text-label font-semibold text-muted">{{ title }}</span>
    <ol class="flex min-w-0 items-center gap-1">
      <li v-for="(name, index) in flow.names" :key="name" class="min-w-0">
        <button
          type="button"
          class="flex h-7 min-w-0 items-center rounded-control px-2 text-label"
          :class="STEP_CLASSES[stateOf(index)]"
          :title="stateOf(index)"
          @click="flow.goTo(index + 1)"
        >
          <span v-if="stateOf(index) === 'feita'">✓</span>
          <span v-else-if="stateOf(index) === 'depois'">{{ index + 1 }}</span>
          <span v-else class="truncate"
            >{{ name }}<template v-if="flow.waitingText !== null"> · {{ flow.waitingText }}</template></span
          >
        </button>
      </li>
    </ol>
    <UiButton v-if="isLast" variant="primary" size="sm" @press="flow.finish()">{{ FLOW_FINISH_LABEL }}</UiButton>
  </div>
</template>
