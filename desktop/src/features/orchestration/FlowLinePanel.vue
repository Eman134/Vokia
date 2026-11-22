<script setup lang="ts">
import UiButton from '../../ui/UiButton.vue'
import FlowLineFields from './FlowLineFields.vue'
import type { LineKind } from './flowGraph'
import { VIEW_COPY } from './orchestrationLabels'
import type { EdgeAction } from './orchestrationTypes'
import type { FlowEdgeMeta } from './useFlowDraft'

// Uma ligação (ou várias, no caso do fim): título, os campos da ligação e a remoção.
export interface LineEditor {
  key: string
  label: string
  meta: FlowEdgeMeta
  kind: LineKind | null
  showEvent: boolean
  showLimit: boolean
  actions: readonly EdgeAction[]
  removable: boolean
}

defineProps<{ title: string; lines: readonly LineEditor[]; pairLimit: number }>()
const emit = defineEmits<{
  close: []
  update: [key: string, patch: Partial<FlowEdgeMeta>]
  setKind: [key: string, kind: LineKind]
  remove: [key: string]
}>()
</script>

<template>
  <section class="flex min-w-0 flex-col gap-4 rounded-md border border-line bg-surface p-4.5">
    <header class="flex items-center justify-between gap-3">
      <h3 class="truncate text-control font-bold text-ink">{{ title }}</h3>
      <UiButton variant="quiet" size="sm" @press="emit('close')">{{ VIEW_COPY.close }}</UiButton>
    </header>
    <p v-if="lines.length === 0" class="text-small text-muted">{{ VIEW_COPY.endNoLines }}</p>
    <div v-for="line in lines" :key="line.key" class="flex min-w-0 flex-col gap-3" :class="lines.length > 1 ? 'rounded-md border border-line p-3' : ''">
      <div class="flex items-center justify-between gap-2">
        <span v-if="lines.length > 1" class="truncate text-control font-semibold text-ink">{{ line.label }}</span>
        <UiButton v-if="line.removable" class="ml-auto" variant="quiet" size="sm" @press="emit('remove', line.key)">
          {{ VIEW_COPY.removeArrow }}
        </UiButton>
      </div>
      <FlowLineFields
        :meta="line.meta"
        :kind="line.kind"
        :show-event="line.showEvent"
        :show-limit="line.showLimit"
        :actions="line.actions"
        :pair-limit="pairLimit"
        @update="emit('update', line.key, $event)"
        @set-kind="emit('setKind', line.key, $event)"
      />
    </div>
  </section>
</template>
