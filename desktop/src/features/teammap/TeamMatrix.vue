<script setup lang="ts">
import { computed } from 'vue'
import type { BacklogItem } from '../backlog/backlogTypes'
import { useBacklog } from '../backlog/useBacklog'
import { useOrchestration } from '../orchestration/useOrchestration'
import type { AgentProfile } from '../team/teamApi'
import { useRoster } from '../team/useRoster'
import AgentCard from './AgentCard.vue'
import { agentCountLabel, CORNER_LABEL, EMPTY_CELL_LABEL, IN_PROGRESS_COLUMN, teamInitial } from './teamMapLayers'

const TEAM_COLUMN_PX = 150
const LAYER_COLUMN_MIN_PX = 170

defineProps<{ highlightedId: string | null }>()
const emit = defineEmits<{ addAgent: [teamId: string, layerId: string] }>()

const orchestration = useOrchestration()
const roster = useRoster()
const backlog = useBacklog()

const layers = computed(() => orchestration.snapshot?.layers ?? [])
const teams = computed(() => orchestration.snapshot?.teams ?? [])
const placements = computed(() => orchestration.snapshot?.placements ?? [])

const placementByAgent = computed(() => new Map(placements.value.map((placement) => [placement.agentId, placement])))

function cellKey(teamId: string, layerId: string): string {
  return `${teamId}|${layerId}`
}

const agentsByCell = computed(() => {
  const cells = new Map<string, AgentProfile[]>()
  for (const agent of roster.agents) {
    const placement = placementByAgent.value.get(agent.id)
    if (placement === undefined) continue
    const key = cellKey(placement.teamId, placement.layerId)
    cells.set(key, [...(cells.get(key) ?? []), agent])
  }
  return cells
})

const agentCountByTeam = computed(() => {
  const counts = new Map<string, number>()
  for (const placement of placements.value) counts.set(placement.teamId, (counts.get(placement.teamId) ?? 0) + 1)
  return counts
})

// Um card por agente: o primeiro em andamento do backlog, na ordem da lista.
const activeCardByAgent = computed(() => {
  const cards = new Map<string, BacklogItem>()
  for (const item of backlog.items) {
    if (item.column === IN_PROGRESS_COLUMN && !cards.has(item.who)) cards.set(item.who, item)
  }
  return cards
})

function cellAgents(teamId: string, layerId: string): AgentProfile[] {
  return agentsByCell.value.get(cellKey(teamId, layerId)) ?? []
}

const gridStyle = computed(() => ({
  gridTemplateColumns: `${TEAM_COLUMN_PX}px repeat(${layers.value.length}, minmax(${LAYER_COLUMN_MIN_PX}px, 1fr))`,
  minWidth: `${TEAM_COLUMN_PX + LAYER_COLUMN_MIN_PX * layers.value.length}px`,
}))
</script>

<template>
  <section aria-label="Mapa" class="overflow-hidden rounded-[24px] border border-line bg-surface">
    <div class="overflow-x-auto">
      <div class="grid" :style="gridStyle">
        <div class="min-w-0 bg-surface-2 px-4.5 py-3.5">
          <span class="text-small text-muted">{{ CORNER_LABEL }}</span>
        </div>
        <div v-for="layer in layers" :key="layer.id" class="flex min-w-0 flex-col bg-surface-2 px-4 py-3.5">
          <span class="text-control font-bold text-ink">{{ layer.name }}</span>
          <span class="text-small text-muted">{{ layer.description }}</span>
        </div>

        <template v-for="team in teams" :key="team.id">
          <div class="flex min-w-0 items-center gap-2.5 border-t border-line px-4.5 py-4">
            <span class="grid size-7.5 shrink-0 place-items-center rounded-[10px] bg-accent-soft text-label font-bold text-accent">
              {{ teamInitial(team.name) }}
            </span>
            <div class="flex min-w-0 flex-col">
              <span class="text-control font-bold text-ink">{{ team.name }}</span>
              <span class="text-small text-muted">{{ agentCountLabel(agentCountByTeam.get(team.id) ?? 0) }}</span>
            </div>
          </div>
          <div
            v-for="layer in layers"
            :key="layer.id"
            class="flex min-w-0 flex-col justify-center gap-2 border-t border-line p-3"
          >
            <AgentCard
              v-for="agent in cellAgents(team.id, layer.id)"
              :key="agent.id"
              :agent="agent"
              :active-card="activeCardByAgent.get(agent.id)"
              :aria-current="agent.id === highlightedId ? 'true' : undefined"
              :class="agent.id === highlightedId ? 'ring-2 ring-accent' : ''"
            />
            <button
              v-if="cellAgents(team.id, layer.id).length === 0"
              type="button"
              class="self-stretch rounded-card border border-dashed border-line py-3 text-center text-small text-muted focus:outline-2 focus:outline-solid focus:outline-accent"
              @click="emit('addAgent', team.id, layer.id)"
            >
              {{ EMPTY_CELL_LABEL }}
            </button>
          </div>
        </template>
      </div>
    </div>
  </section>
</template>
