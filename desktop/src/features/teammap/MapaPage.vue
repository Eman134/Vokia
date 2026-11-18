<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import OrchestrationView from '../orchestration/OrchestrationView.vue'
import { useOrchestration } from '../orchestration/useOrchestration'
import { useProjects } from '../projects/useProjects'
import type { Agent } from '../team/events'
import type { AgentProfile } from '../team/teamApi'
import { placeableAgents } from '../team/agents'
import { useRoster } from '../team/useRoster'
import FlowRules from './FlowRules.vue'
import MovementList from './MovementList.vue'
import NewLayerDialog from './NewLayerDialog.vue'
import NewTeamDialog from './NewTeamDialog.vue'
import PlaceAgentDialog from './PlaceAgentDialog.vue'
import LinkAgentDialog from './LinkAgentDialog.vue'
import TeamMatrix from './TeamMatrix.vue'
import UnplacedAgents from './UnplacedAgents.vue'
import {
  ADD_LAYER_LABEL,
  ADD_TEAM_LABEL,
  CREATE_LAYER_LABEL,
  CREATE_TEAM_LABEL,
  LOAD_ERROR_TITLE,
  MAP_SUBTITLE,
  MAP_TITLE,
  NO_LAYERS_TITLE,
  NO_PROJECT_TITLE,
  NO_TEAMS_TITLE,
  TAB_ORCHESTRATION,
  TAB_TEAMS,
} from './teamMapLayers'

const PAGE_CLASSES = 'flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto pt-7 pr-8 pb-[150px] pl-2'
const TAB_LIST_CLASSES = 'flex gap-0.5 rounded-control-lg bg-surface-2 p-0.75'
const TAB_BASE_CLASSES = 'rounded-[9px] px-3 py-1.5 text-caption disabled:opacity-40'
const TAB_ACTIVE_CLASSES = 'bg-surface font-semibold text-ink shadow-seg'
const TAB_INACTIVE_CLASSES = 'text-muted'
const EMPTY_ACTION_CLASSES = 'flex flex-col items-center gap-3'

const orchestration = useOrchestration()
const projects = useProjects()
const roster = useRoster()

const snapshot = computed(() => orchestration.snapshot)
const projectId = computed(() => orchestration.loadedProjectId)
const linkableAgents = computed<AgentProfile[]>(() => placeableAgents(roster.agents))
const unplaced = computed<AgentProfile[]>(() => {
  const placed = new Set((snapshot.value?.placements ?? []).map((placement) => placement.agentId))
  return linkableAgents.value.filter((agent) => !placed.has(agent.id))
})

type MapTab = 'teams' | 'orchestration'

// Aba ativa só vale enquanto a página está aberta: volta a "Times e camadas" ao reabrir o mapa.
const activeTab = ref<MapTab>('teams')
const teamDialogOpen = ref(false)
const layerDialogOpen = ref(false)
const placing = ref<AgentProfile | null>(null)
const linkingCell = ref<{ teamId: string; layerId: string; replacingId: Agent | null } | null>(null)

function openLink(teamId: string, layerId: string): void {
  linkingCell.value = { teamId, layerId, replacingId: null }
}

function openReplace(teamId: string, layerId: string, agentId: Agent): void {
  linkingCell.value = { teamId, layerId, replacingId: agentId }
}
</script>

<template>
  <section :class="PAGE_CLASSES">
    <header class="flex flex-wrap items-end gap-4">
      <div>
        <h1 class="mt-1 text-[32px] font-bold tracking-[-0.03em] text-ink">{{ MAP_TITLE }}</h1>
        <p class="mt-1 max-w-[64ch] text-body text-muted">{{ MAP_SUBTITLE }}</p>
      </div>
      <div class="ml-auto flex flex-wrap items-center gap-2">
        <div role="tablist" :class="TAB_LIST_CLASSES">
          <button
            type="button"
            role="tab"
            :aria-selected="activeTab === 'teams'"
            :class="[TAB_BASE_CLASSES, activeTab === 'teams' ? TAB_ACTIVE_CLASSES : TAB_INACTIVE_CLASSES]"
            @click="activeTab = 'teams'"
          >
            {{ TAB_TEAMS }}
          </button>
          <button
            type="button"
            role="tab"
            :aria-selected="activeTab === 'orchestration'"
            :class="[TAB_BASE_CLASSES, activeTab === 'orchestration' ? TAB_ACTIVE_CLASSES : TAB_INACTIVE_CLASSES]"
            @click="activeTab = 'orchestration'"
          >
            {{ TAB_ORCHESTRATION }}
          </button>
        </div>
        <UiButton variant="quiet" size="lg" :disabled="projectId === null" @press="teamDialogOpen = true">
          {{ ADD_TEAM_LABEL }}
        </UiButton>
        <UiButton variant="primary" size="lg" :disabled="projectId === null" @press="layerDialogOpen = true">
          {{ ADD_LAYER_LABEL }}
        </UiButton>
      </div>
    </header>

    <UiEmpty v-if="!projects.selected" :title="NO_PROJECT_TITLE" />
    <UiEmpty v-else-if="orchestration.loadError" :title="LOAD_ERROR_TITLE" :text="orchestration.loadError" />
    <template v-else-if="snapshot && activeTab === 'teams'">
      <div v-if="snapshot.layers.length === 0" :class="EMPTY_ACTION_CLASSES">
        <UiEmpty :title="NO_LAYERS_TITLE" />
        <UiButton variant="primary" size="lg" @press="layerDialogOpen = true">{{ CREATE_LAYER_LABEL }}</UiButton>
      </div>
      <div v-else-if="snapshot.teams.length === 0" :class="EMPTY_ACTION_CLASSES">
        <UiEmpty :title="NO_TEAMS_TITLE" />
        <UiButton variant="primary" size="lg" @press="teamDialogOpen = true">{{ CREATE_TEAM_LABEL }}</UiButton>
      </div>
      <TeamMatrix v-else :can-link="linkableAgents.length > 0" @link="openLink" @replace="openReplace" />
      <UnplacedAgents v-if="unplaced.length > 0" :agents="unplaced" @place="placing = $event" />
      <div class="grid gap-5 lg:grid-cols-2">
        <MovementList :movements="orchestration.movements" :rules="snapshot.rules" />
        <FlowRules
          :rules="snapshot.rules"
          :layers="snapshot.layers"
          :enforcement="snapshot.settings.enforcement"
          @edit="activeTab = 'orchestration'"
        />
      </div>
    </template>
    <OrchestrationView v-else-if="snapshot" />

    <template v-if="projectId !== null">
      <NewTeamDialog v-if="teamDialogOpen" :project-id="projectId" @close="teamDialogOpen = false" />
      <NewLayerDialog v-if="layerDialogOpen" :project-id="projectId" @close="layerDialogOpen = false" />
      <PlaceAgentDialog v-if="placing" :project-id="projectId" :agent="placing" @close="placing = null" />
      <LinkAgentDialog
        v-if="linkingCell"
        :project-id="projectId"
        :team-id="linkingCell.teamId"
        :layer-id="linkingCell.layerId"
        :replacing-id="linkingCell.replacingId"
        :agents="linkableAgents"
        @close="linkingCell = null"
      />
    </template>
  </section>
</template>
