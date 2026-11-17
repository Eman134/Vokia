<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import NewLayerDialog from '../teammap/NewLayerDialog.vue'
import AgentPortrait from '../team/AgentPortrait.vue'
import type { Agent } from '../team/events'
import type { AgentProfile } from '../team/teamApi'
import { useRoster } from '../team/useRoster'
import AgentPanel from './AgentPanel.vue'
import FlowDiagram from './FlowDiagram.vue'
import LayerPanel from './LayerPanel.vue'
import OrchestrationPanel from './OrchestrationPanel.vue'
import RulePanel from './RulePanel.vue'
import { VIEW_COPY } from './orchestrationLabels'
import type { OrchRule, OrchestrationSnapshot } from './orchestrationTypes'
import { useOrchestration } from './useOrchestration'

const PORTRAIT_SIZE = 28

type Selection = { kind: 'rule'; id: number } | { kind: 'layer'; id: string } | { kind: 'agent'; id: Agent }

const orchestration = useOrchestration()
const roster = useRoster()

const selection = ref<Selection | null>(null)
const creating = ref(false)
const pendingRuleId = ref<number | null>(null)
const layerDialogOpen = ref(false)

const snapshot = computed<OrchestrationSnapshot | null>(() => orchestration.snapshot)
const layers = computed(() => snapshot.value?.layers ?? [])
const rules = computed(() => snapshot.value?.rules ?? [])
const orderedLayerIds = computed(() =>
  [...layers.value].sort((a, b) => a.position - b.position).map((layer) => layer.id),
)

const selectedRule = computed<OrchRule | null>(() =>
  selection.value?.kind === 'rule' ? (rules.value.find((rule) => rule.id === selection.value?.id) ?? null) : null,
)
const selectedLayer = computed(() =>
  selection.value?.kind === 'layer' ? (layers.value.find((layer) => layer.id === selection.value?.id) ?? null) : null,
)
const selectedAgent = computed<AgentProfile | null>(() =>
  selection.value?.kind === 'agent' ? (roster.profileOf(selection.value.id) ?? null) : null,
)
const selectedRuleId = computed(() => (selection.value?.kind === 'rule' ? selection.value.id : null))
const selectedLayerId = computed(() => (selection.value?.kind === 'layer' ? selection.value.id : null))

function placementOf(agentId: Agent) {
  return snapshot.value?.placements.find((placement) => placement.agentId === agentId)
}

function placementLabel(agentId: Agent): string {
  const placement = placementOf(agentId)
  if (!placement) return VIEW_COPY.unplaced
  const team = snapshot.value?.teams.find((candidate) => candidate.id === placement.teamId)?.name ?? ''
  const layer = layers.value.find((candidate) => candidate.id === placement.layerId)?.name ?? ''
  return `${team} · ${layer}`
}

function clearSelection(): void {
  selection.value = null
  creating.value = false
}

function selectRule(id: number): void {
  creating.value = false
  selection.value = { kind: 'rule', id }
}

function selectLayer(id: string): void {
  creating.value = false
  selection.value = { kind: 'layer', id }
}

function selectAgent(id: Agent): void {
  creating.value = false
  selection.value = { kind: 'agent', id }
}

function openNewArrow(): void {
  selection.value = null
  creating.value = true
}

function onRuleCreated(rule: OrchRule): void {
  if (rules.value.some((candidate) => candidate.id === rule.id)) {
    selectRule(rule.id)
    return
  }
  pendingRuleId.value = rule.id
}

// Seleção só vale enquanto o item existe no snapshot; outra janela pode removê-lo.
watch(snapshot, (next) => {
  if (next === null) {
    pendingRuleId.value = null
    clearSelection()
    return
  }
  if (pendingRuleId.value !== null && next.rules.some((rule) => rule.id === pendingRuleId.value)) {
    selectRule(pendingRuleId.value)
    pendingRuleId.value = null
  }
  if (selection.value?.kind === 'rule' && selectedRule.value === null) selection.value = null
  if (selection.value?.kind === 'layer' && selectedLayer.value === null) selection.value = null
})

watch(
  () => orchestration.loadedProjectId,
  () => {
    pendingRuleId.value = null
    clearSelection()
  },
)

function onKeydown(event: KeyboardEvent): void {
  if (event.key !== 'Escape') return
  if (creating.value || selection.value !== null) {
    event.preventDefault()
    clearSelection()
  }
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <section v-if="snapshot" class="grid min-w-0 gap-5 lg:grid-cols-[minmax(0,1fr)_minmax(320px,400px)]">
    <div class="flex min-w-0 flex-col gap-4">
      <header class="flex items-center justify-between gap-3">
        <h2 class="text-control font-bold text-ink">{{ VIEW_COPY.flowTitle }}</h2>
        <UiButton v-if="layers.length > 0" variant="primary" size="sm" @press="openNewArrow()">
          {{ VIEW_COPY.addArrow }}
        </UiButton>
      </header>

      <div v-if="layers.length === 0" class="flex flex-col items-center gap-3">
        <UiEmpty :title="VIEW_COPY.noLayersTitle" />
        <UiButton variant="primary" size="lg" @press="layerDialogOpen = true">{{ VIEW_COPY.createLayer }}</UiButton>
      </div>
      <FlowDiagram
        v-else
        :layers="layers"
        :rules="rules"
        :human-gate="snapshot.settings.humanGate"
        :selected-rule-id="selectedRuleId"
        :selected-layer-id="selectedLayerId"
        @select-rule="selectRule"
        @select-layer="selectLayer"
      />

      <ul class="m-0 flex list-none flex-col gap-1.5 p-0">
        <li v-for="agent in roster.agents" :key="agent.id">
          <button
            type="button"
            class="flex w-full min-w-0 items-center gap-2.5 rounded-control px-2.5 py-2 text-left"
            :class="selection?.kind === 'agent' && selection.id === agent.id ? 'bg-accent-soft' : ''"
            @click="selectAgent(agent.id)"
          >
            <AgentPortrait :who="agent.id" :size="PORTRAIT_SIZE" />
            <span class="truncate text-control font-semibold text-ink">{{ agent.name }}</span>
            <span class="ml-auto truncate text-small text-muted">{{ placementLabel(agent.id) }}</span>
          </button>
        </li>
      </ul>
    </div>

    <aside class="min-w-0">
      <RulePanel
        v-if="creating || selectedRule"
        :key="creating ? 'new-arrow' : `rule-${selectedRule?.id}`"
        :rule="selectedRule"
        :layers="layers"
        @close="clearSelection()"
        @created="onRuleCreated"
      />
      <LayerPanel
        v-else-if="selectedLayer"
        :key="selectedLayer.id"
        :layer="selectedLayer"
        :ordered-ids="orderedLayerIds"
        @close="clearSelection()"
      />
      <AgentPanel
        v-else-if="selectedAgent"
        :key="selectedAgent.id"
        :agent="selectedAgent"
        :placement="placementOf(selectedAgent.id)"
        :teams="snapshot.teams"
        :layers="layers"
        @close="clearSelection()"
      />
      <OrchestrationPanel v-else :settings="snapshot.settings" :layers="layers" :rules="rules" />
    </aside>

    <NewLayerDialog
      v-if="layerDialogOpen && orchestration.loadedProjectId !== null"
      :project-id="orchestration.loadedProjectId"
      @close="layerDialogOpen = false"
    />
  </section>
</template>
