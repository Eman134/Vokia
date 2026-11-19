<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import { flowSourceFrom } from '../orchestration/flowSync'
import ModalPanel from '../orchestration/ModalPanel.vue'
import OrchestrationView from '../orchestration/OrchestrationView.vue'
import AgentActivityPanel from './AgentActivityPanel.vue'
import TeamsList from '../orchestration/TeamsList.vue'
import { VIEW_COPY } from '../orchestration/orchestrationLabels'
import { peekFlowDraft } from '../orchestration/useFlowDraft'
import { useOrchestration } from '../orchestration/useOrchestration'
import { MAP_TITLE } from './teamMapLayers'

const PAGE_CLASSES = `flex min-h-0 min-w-0 flex-1 flex-col gap-5 overflow-hidden ${PAGE_INSET_CLASSES}`

const orchestration = useOrchestration()
const selectedTeamId = ref<string | null>(null)
const selectedTeam = computed(
  () => orchestration.snapshot?.teams.find((team) => team.id === selectedTeamId.value) ?? null,
)
// Agentes posicionados em algum time: a atividade do mapa mostra todos eles.
const placedAgents = computed(() => [...new Set((orchestration.snapshot?.placements ?? []).map((placement) => placement.agentId))])
const confirmingLeave = ref(false)

// Sair do time com o fluxo alterado e não salvo pede confirmação; descartar volta o desenho ao que está gravado.
function backToTeams(): void {
  const teamId = selectedTeamId.value
  if (teamId !== null && peekFlowDraft(teamId)?.dirty.value) {
    confirmingLeave.value = true
    return
  }
  selectedTeamId.value = null
}

function discardAndLeave(): void {
  const teamId = selectedTeamId.value
  const snapshot = orchestration.snapshot
  if (teamId !== null && snapshot !== null) peekFlowDraft(teamId)?.load(flowSourceFrom(snapshot, teamId))
  confirmingLeave.value = false
  selectedTeamId.value = null
}
</script>

<template>
  <section :class="PAGE_CLASSES">
    <UiPageHeader :title="selectedTeam ? MAP_TITLE : VIEW_COPY.teamsTitle" />
    <UiEmpty v-if="orchestration.snapshot === null" :title="VIEW_COPY.loadingTeams" />
    <template v-else-if="selectedTeam">
      <div class="flex min-w-0 items-center gap-3">
        <UiButton variant="quiet" size="sm" @press="backToTeams">{{ VIEW_COPY.backToTeams }}</UiButton>
        <h2 class="min-w-0 truncate text-control font-bold text-ink">{{ selectedTeam.name }}</h2>
      </div>
      <OrchestrationView :key="selectedTeam.id" :team-id="selectedTeam.id" />
    </template>
    <template v-else>
      <TeamsList @open="selectedTeamId = $event" />
      <AgentActivityPanel :agents="placedAgents" />
    </template>
    <ModalPanel v-if="confirmingLeave" :title="VIEW_COPY.discardTitle" @close="confirmingLeave = false">
      <div class="flex justify-end gap-2">
        <UiButton variant="quiet" size="lg" @press="confirmingLeave = false">{{ VIEW_COPY.discardKeep }}</UiButton>
        <UiButton variant="danger" size="lg" @press="discardAndLeave">{{ VIEW_COPY.discardConfirm }}</UiButton>
      </div>
    </ModalPanel>
  </section>
</template>
