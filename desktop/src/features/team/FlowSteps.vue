<script setup lang="ts">
import { useFlow } from './useFlow'

type StepState = 'feita' | 'atual' | 'depois'

const STEP_CLASSES: Record<StepState, string> = {
  feita: 'text-ink',
  atual: 'bg-ink text-bg font-semibold',
  depois: 'text-muted',
}
const DOT_CLASSES: Record<StepState, string> = {
  feita: 'bg-accent-soft text-accent',
  atual: 'bg-bg text-ink',
  depois: 'border border-line text-muted',
}

const flow = useFlow()

function stateOf(index: number): StepState {
  const number = index + 1
  if (number < flow.step) {
    return 'feita'
  }
  return number === flow.step ? 'atual' : 'depois'
}

function stepTitle(name: string, index: number): string {
  const state = stateOf(index)
  const suffix = state === 'feita' ? ' · feita' : state === 'atual' ? ' · agora' : ''
  return `${index + 1}. ${name}${suffix}`
}
</script>

<template>
  <div class="flex min-w-0 items-center">
    <ol class="flex min-w-0 items-center gap-1 max-md:gap-0.5">
      <li v-for="(name, index) in flow.names" :key="name" class="min-w-0">
        <button
          type="button"
          class="flex h-7 min-w-0 items-center gap-1.5 rounded-control px-0.5 text-label"
          :class="[STEP_CLASSES[stateOf(index)], stateOf(index) === 'atual' ? 'pr-2.5 max-lg:pr-0.5' : '']"
          :title="stepTitle(name, index)"
          :aria-current="stateOf(index) === 'atual' ? 'step' : undefined"
          :disabled="stateOf(index) !== 'feita'"
          @click="flow.goTo(index + 1)"
        >
          <span
            class="grid size-5.5 shrink-0 place-items-center rounded-full text-small font-bold"
            :class="DOT_CLASSES[stateOf(index)]"
          >{{ stateOf(index) === 'feita' ? '✓' : index + 1 }}</span>
          <span v-if="stateOf(index) === 'atual'" class="truncate pr-1 max-lg:hidden">{{ name }}</span>
        </button>
      </li>
    </ol>
  </div>
</template>
