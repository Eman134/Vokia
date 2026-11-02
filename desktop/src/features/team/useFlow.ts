import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { FLOW_STEPS, type FlowId } from './flowSteps'

const FIRST_STEP = 1

export const useFlow = defineStore('flow', () => {
  const flow = ref<FlowId | null>(null)
  const step = ref(FIRST_STEP)
  const names = computed<readonly string[]>(() => (flow.value === null ? [] : FLOW_STEPS[flow.value]))
  const active = computed(() => flow.value !== null)
  const total = computed(() => names.value.length)
  const progress = computed(() => (total.value === 0 ? 0 : step.value / total.value))

  function isStep(value: number): boolean {
    return Number.isInteger(value) && value >= FIRST_STEP && value <= total.value
  }

  function start(next: FlowId, first: number = FIRST_STEP): void {
    flow.value = next
    step.value = isStep(first) ? first : FIRST_STEP
  }

  function goTo(target: number): void {
    if (active.value && isStep(target)) {
      step.value = target
    }
  }

  function finish(): void {
    flow.value = null
    step.value = FIRST_STEP
  }

  return { flow, step, names, active, total, progress, start, goTo, finish }
})
