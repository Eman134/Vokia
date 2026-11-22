<script setup lang="ts">
import { computed, ref } from 'vue'
import UiMarker from '../../ui/UiMarker.vue'
import { HAND_SELECTED_CLASSES, INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import AgentPortrait from '../team/AgentPortrait.vue'
import { DEFAULT_RECIPIENT } from '../team/agents'
import type { Agent } from '../team/events'
import { useRoster } from '../team/useRoster'
import { useItemPlan } from './useItemPlan'

const model = defineModel<Agent>({ required: true })
const emit = defineEmits<{ chosen: [] }>()

const CAST_LABEL = 'Quem planeja'
const RECOMMENDED_LABEL = 'recomendado'
const BUSY_LABEL = 'escrevendo outro plano'
const BUSY_TITLE = 'Este agente já está escrevendo um plano. Espere ele terminar ou escolha outro.'
const PORTRAIT_SIZE = 28
const OPTION_CLASSES = `flex w-full items-center gap-2.5 rounded-md border-[1.5px] border-transparent px-2 py-1.5 text-left ${INK_FOCUS_CLASSES}`
const OPTION_ENABLED = 'cursor-pointer hover:bg-surface-2'
const OPTION_BUSY = 'cursor-not-allowed opacity-50'
const NEXT_KEYS = ['ArrowDown', 'ArrowRight']
const PREVIOUS_KEYS = ['ArrowUp', 'ArrowLeft']

const roster = useRoster()
const itemPlan = useItemPlan()
const list = ref<HTMLElement>()

const options = computed(() =>
  roster.agents.map((profile) => ({
    profile,
    busy: itemPlan.isAgentAwaiting(profile.id),
    recommended: profile.id === DEFAULT_RECIPIENT,
  })),
)

function choose(agent: Agent, busy: boolean): void {
  if (busy) return
  model.value = agent
  emit('chosen')
}

// Setas andam entre os livres como num radiogroup nativo; a escolha só fecha a lista no Enter ou no clique.
function move(event: KeyboardEvent): void {
  const step = NEXT_KEYS.includes(event.key) ? 1 : PREVIOUS_KEYS.includes(event.key) ? -1 : 0
  if (step === 0) return
  event.preventDefault()
  const free = options.value.filter((option) => !option.busy)
  if (free.length === 0) return
  const index = free.findIndex((option) => option.profile.id === model.value)
  const next = free[(index + step + free.length) % free.length]
  model.value = next.profile.id
  list.value?.querySelector<HTMLElement>(`[data-agent="${next.profile.id}"]`)?.focus()
}
</script>

<template>
  <div ref="list" role="radiogroup" :aria-label="CAST_LABEL" class="flex flex-col gap-0.5" @keydown="move">
    <button
      v-for="option in options"
      :key="option.profile.id"
      type="button"
      role="radio"
      :data-agent="option.profile.id"
      :aria-checked="model === option.profile.id"
      :aria-disabled="option.busy"
      :tabindex="model === option.profile.id ? 0 : -1"
      :title="option.busy ? BUSY_TITLE : undefined"
      :class="[OPTION_CLASSES, option.busy ? OPTION_BUSY : OPTION_ENABLED, model === option.profile.id && HAND_SELECTED_CLASSES]"
      @click="choose(option.profile.id, option.busy)"
    >
      <AgentPortrait :who="option.profile.id" :size="PORTRAIT_SIZE" class="shrink-0" />
      <span class="flex min-w-0 flex-col">
        <span class="flex items-center gap-1.5 text-control font-medium text-ink">
          {{ option.profile.name }}
          <UiMarker v-if="option.recommended" class="text-meta">{{ RECOMMENDED_LABEL }}</UiMarker>
        </span>
        <span class="truncate text-meta text-muted">{{ option.busy ? BUSY_LABEL : option.profile.role || option.profile.model }}</span>
      </span>
    </button>
  </div>
</template>
