<script setup lang="ts">
import { computed } from 'vue'
import type { AgentProfile } from '../team/teamApi'
import { useRoster } from '../team/useRoster'
import AgentCard from './AgentCard.vue'
import {
  agentCountLabel,
  CORNER_LABEL,
  EMPTY_CELL_LABEL,
  layerOf,
  MAP_LAYERS,
  TEAM_ROW_INITIAL,
  TEAM_ROW_NAME,
  type MapLayerId,
} from './teamMapLayers'

const TEAM_COLUMN_PX = 150
const LAYER_COLUMN_MIN_PX = 170

defineProps<{ highlightedId: string | null }>()
const emit = defineEmits<{ addAgent: [] }>()

const roster = useRoster()

const agentsByLayer = computed<Record<MapLayerId, AgentProfile[]>>(() => {
  const grouped: Record<MapLayerId, AgentProfile[]> = { lideranca: [], implementacao: [], revisao: [] }
  for (const agent of roster.agents) grouped[layerOf(agent.id)].push(agent)
  return grouped
})

const gridStyle = {
  gridTemplateColumns: `${TEAM_COLUMN_PX}px repeat(${MAP_LAYERS.length}, minmax(${LAYER_COLUMN_MIN_PX}px, 1fr))`,
  minWidth: `${TEAM_COLUMN_PX + LAYER_COLUMN_MIN_PX * MAP_LAYERS.length}px`,
}
</script>

<template>
  <section aria-label="Mapa" class="overflow-hidden rounded-[24px] border border-line bg-surface">
    <div class="overflow-x-auto">
      <div class="grid" :style="gridStyle">
        <div class="min-w-0 bg-surface-2 px-4.5 py-3.5">
          <span class="text-small text-muted">{{ CORNER_LABEL }}</span>
        </div>
        <div v-for="layer in MAP_LAYERS" :key="layer.id" class="flex min-w-0 flex-col bg-surface-2 px-4 py-3.5">
          <span class="text-control font-bold text-ink">{{ layer.title }}</span>
          <span class="text-small text-muted">{{ layer.description }}</span>
        </div>

        <div class="flex min-w-0 items-center gap-2.5 border-t border-line px-4.5 py-4">
          <span class="grid size-7.5 shrink-0 place-items-center rounded-[10px] bg-accent-soft text-label font-bold text-accent">
            {{ TEAM_ROW_INITIAL }}
          </span>
          <div class="flex min-w-0 flex-col">
            <span class="text-control font-bold text-ink">{{ TEAM_ROW_NAME }}</span>
            <span class="text-small text-muted">{{ agentCountLabel(roster.agents.length) }}</span>
          </div>
        </div>
        <div
          v-for="layer in MAP_LAYERS"
          :key="layer.id"
          class="flex min-w-0 flex-col justify-center gap-2 border-t border-line p-3"
        >
          <AgentCard
            v-for="agent in agentsByLayer[layer.id]"
            :key="agent.id"
            :agent="agent"
            :aria-current="agent.id === highlightedId ? 'true' : undefined"
            :class="agent.id === highlightedId ? 'ring-2 ring-accent' : ''"
          />
          <button
            v-if="agentsByLayer[layer.id].length === 0"
            type="button"
            class="self-stretch rounded-card border border-dashed border-line py-3 text-center text-small text-muted focus:outline-2 focus:outline-solid focus:outline-accent"
            @click="emit('addAgent')"
          >
            {{ EMPTY_CELL_LABEL }}
          </button>
        </div>
      </div>
    </div>
  </section>
</template>
