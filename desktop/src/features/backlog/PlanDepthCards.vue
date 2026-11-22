<script lang="ts">
import type { PlanDepth } from './useItemPlan'

export const DEPTH_LABELS: Record<PlanDepth, string> = {
  rapido: 'rápido',
  completo: 'completo',
}
</script>

<script setup lang="ts">
import { ref } from 'vue'
import { HAND_SELECTED_CLASSES, INK_FOCUS_CLASSES } from '../../ui/handDrawn'

const model = defineModel<PlanDepth>({ required: true })
const emit = defineEmits<{ chosen: [] }>()

const DEPTH_LABEL = 'Profundidade do plano'
const DEPTH_OPTIONS: { id: PlanDepth; title: string; effect: string }[] = [
  { id: 'rapido', title: 'Rápido', effect: 'Um lote, sem pesquisa. Bom para ajustes pequenos.' },
  { id: 'completo', title: 'Completo', effect: 'Lê o código e a memória e quebra em vários lotes.' },
]
const CARD_CLASSES = `flex cursor-pointer flex-col gap-1 rounded-md border-[1.5px] border-line p-2.5 text-left hover:bg-surface-2 ${INK_FOCUS_CLASSES}`
const SWITCH_KEYS = ['ArrowDown', 'ArrowRight', 'ArrowUp', 'ArrowLeft']

const list = ref<HTMLElement>()

function choose(depth: PlanDepth): void {
  model.value = depth
  emit('chosen')
}

// Com duas opções, qualquer seta troca para a outra, como num radiogroup nativo.
function move(event: KeyboardEvent): void {
  if (!SWITCH_KEYS.includes(event.key)) return
  event.preventDefault()
  const next = DEPTH_OPTIONS.find((option) => option.id !== model.value)
  if (!next) return
  model.value = next.id
  list.value?.querySelector<HTMLElement>(`[data-depth="${next.id}"]`)?.focus()
}
</script>

<template>
  <div ref="list" role="radiogroup" :aria-label="DEPTH_LABEL" class="grid grid-cols-2 gap-2" @keydown="move">
    <button
      v-for="option in DEPTH_OPTIONS"
      :key="option.id"
      type="button"
      role="radio"
      :data-depth="option.id"
      :aria-checked="model === option.id"
      :tabindex="model === option.id ? 0 : -1"
      :class="[CARD_CLASSES, model === option.id && HAND_SELECTED_CLASSES]"
      @click="choose(option.id)"
    >
      <span class="text-control font-semibold text-ink">{{ option.title }}</span>
      <span class="text-meta text-muted">{{ option.effect }}</span>
    </button>
  </div>
</template>
