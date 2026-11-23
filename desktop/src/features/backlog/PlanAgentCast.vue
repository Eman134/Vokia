<script setup lang="ts">
import { computed, ref } from 'vue'
import UiMarker from '../../ui/UiMarker.vue'
import { HAND_SELECTED_CLASSES, INK_FOCUS_CLASSES } from '../../ui/handDrawn'
import AgentPortrait from '../team/AgentPortrait.vue'
import { DEFAULT_RECIPIENT } from '../team/agents'
import type { Agent } from '../team/events'
import { useRoster } from '../team/useRoster'

const model = defineModel<Agent>({ required: true })
const emit = defineEmits<{ chosen: [] }>()

const CAST_LABEL = 'Quem planeja'
const RECOMMENDED_LABEL = 'recomendado'
const PORTRAIT_SIZE = 28
const OPTION_CLASSES = `flex w-full items-center gap-2.5 rounded-md border-[1.5px] border-transparent px-2 py-1.5 text-left ${INK_FOCUS_CLASSES}`
const OPTION_ENABLED = 'cursor-pointer hover:bg-surface-2'
const NEXT_KEYS = ['ArrowDown', 'ArrowRight']
const PREVIOUS_KEYS = ['ArrowUp', 'ArrowLeft']

const roster = useRoster()
const list = ref<HTMLElement>()

const options = computed(() =>
  roster.agents.map((profile) => ({
    profile,
    recommended: profile.id === DEFAULT_RECIPIENT,
  })),
)

function choose(agent: Agent): void {
  model.value = agent
  emit('chosen')
}

// Setas andam entre as opções como num radiogroup nativo; a escolha só fecha a lista no Enter ou no clique.
function move(event: KeyboardEvent): void {
  const step = NEXT_KEYS.includes(event.key) ? 1 : PREVIOUS_KEYS.includes(event.key) ? -1 : 0
  if (step === 0) return
  event.preventDefault()
  const available = options.value
  if (available.length === 0) return
  const index = available.findIndex((option) => option.profile.id === model.value)
  const next = available[(index + step + available.length) % available.length]
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
      :tabindex="model === option.profile.id ? 0 : -1"
      :class="[OPTION_CLASSES, OPTION_ENABLED, model === option.profile.id && HAND_SELECTED_CLASSES]"
      @click="choose(option.profile.id)"
    >
      <AgentPortrait :who="option.profile.id" :size="PORTRAIT_SIZE" class="shrink-0" />
      <span class="flex min-w-0 flex-col">
        <span class="flex items-center gap-1.5 text-control font-medium text-ink">
          {{ option.profile.name }}
          <UiMarker v-if="option.recommended" class="text-meta">{{ RECOMMENDED_LABEL }}</UiMarker>
        </span>
        <span class="truncate text-meta text-muted">{{ option.profile.role || option.profile.model }}</span>
      </span>
    </button>
  </div>
</template>
