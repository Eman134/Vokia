<script setup lang="ts">
import { computed } from 'vue'
import UiSegmented from '../../ui/UiSegmented.vue'
import UiSelect from '../../ui/UiSelect.vue'
import CountStepper from './CountStepper.vue'
import type { FlowEdgeMeta } from './useFlowDraft'
import { INSTRUCTIONS_MAX_CHARS, TEXTAREA_CLASSES } from './orchestrationForm'
import { EVENT_OPTIONS, VIEW_COPY } from './orchestrationLabels'
import { EDGE_ACTION_LABELS } from './orchestrationLabels'
import type { EdgeAction, WhenEvent } from './orchestrationTypes'
import type { LineKind } from './flowGraph'

const TURN_LIMIT_MIN = 0
const TURN_LIMIT_MAX = 50

const props = defineProps<{
  meta: FlowEdgeMeta
  // Tipo da linha (síncrona ou assíncrona): só em ligação entre agentes; null esconde.
  kind: LineKind | null
  showEvent: boolean
  showLimit: boolean
  // Ações permitidas na ligação (a do fim é só escalar); uma só fica travada.
  actions: readonly EdgeAction[]
  pairLimit: number
}>()
const emit = defineEmits<{ update: [patch: Partial<FlowEdgeMeta>]; setKind: [kind: LineKind] }>()

const KIND_OPTIONS = [
  { value: 'sync' as const, label: VIEW_COPY.lineSync },
  { value: 'async' as const, label: VIEW_COPY.lineAsync },
]
const actionOptions = computed(() => props.actions.map((value) => ({ value, label: EDGE_ACTION_LABELS[value] })))
const instructionsCount = computed(() => `${props.meta.instructions.length} / ${INSTRUCTIONS_MAX_CHARS}`)
// 0 no seletor é "padrão": o limite do mapa (pair_turn_limit) vale.
const limitModel = computed({
  get: () => props.meta.turnLimit ?? 0,
  set: (value: number) => emit('update', { turnLimit: value === 0 ? null : value }),
})
</script>

<template>
  <div class="flex min-w-0 flex-col gap-3">
    <div v-if="kind !== null" class="flex flex-col gap-1.5">
      <span class="text-label font-medium text-muted">{{ VIEW_COPY.lineTypeLabel }}</span>
      <UiSegmented :model-value="kind" :options="KIND_OPTIONS" :label="VIEW_COPY.lineTypeLabel" @update:model-value="emit('setKind', $event)" />
    </div>
    <div v-if="showEvent" class="flex items-center justify-between gap-2">
      <span class="text-label font-medium text-muted">{{ VIEW_COPY.exitEvent }}</span>
      <UiSelect
        :model-value="meta.whenEvent"
        :options="EVENT_OPTIONS"
        :label="VIEW_COPY.exitEvent"
        @update:model-value="emit('update', { whenEvent: $event as WhenEvent })"
      />
    </div>
    <div v-if="actionOptions.length > 0" class="flex items-center justify-between gap-2">
      <span class="text-label font-medium text-muted">{{ VIEW_COPY.lineAction }}</span>
      <UiSelect
        :model-value="meta.action"
        :options="actionOptions"
        :label="VIEW_COPY.lineAction"
        :disabled="actionOptions.length === 1"
        @update:model-value="emit('update', { action: $event as EdgeAction })"
      />
    </div>
    <div v-if="showLimit" class="flex items-center justify-between gap-2">
      <span class="text-label font-medium text-muted">{{ VIEW_COPY.turnLimit }}</span>
      <div class="flex items-center gap-2">
        <CountStepper v-model="limitModel" :label="VIEW_COPY.turnLimit" :min="TURN_LIMIT_MIN" :max="TURN_LIMIT_MAX" />
        <span v-if="limitModel === 0" class="text-small text-muted">{{ VIEW_COPY.turnLimitDefault(pairLimit) }}</span>
      </div>
    </div>
    <label class="flex flex-col gap-1.5">
      <span class="flex items-baseline justify-between gap-3 text-label font-medium text-muted">
        {{ VIEW_COPY.lineInstruction }}
        <span class="font-mono text-tiny font-normal text-faint">{{ instructionsCount }}</span>
      </span>
      <textarea
        :value="meta.instructions"
        rows="4"
        :class="TEXTAREA_CLASSES"
        :placeholder="VIEW_COPY.lineInstructionHint"
        @input="emit('update', { instructions: ($event.target as HTMLTextAreaElement).value })"
      />
    </label>
  </div>
</template>
