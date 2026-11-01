<script setup lang="ts">
import { computed } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import { PAUSED_BADGE, pausedEntryLabel } from '../panel/nowCopy'
import { useAgentControl } from '../panel/useAgentControl'
import { usePanelTab } from '../panel/usePanelTab'
import { useProjects } from '../projects/useProjects'
import { agentName } from './useTeam'

const control = useAgentControl()
const projects = useProjects()
const panelTab = usePanelTab()

const description = computed(() =>
  control.pausedList
    .map((pause) =>
      pausedEntryLabel(agentName(pause.agent), projects.projects.find((project) => project.id === pause.projectId)?.name),
    )
    .join(', '),
)
</script>

<template>
  <UiButton
    v-if="control.pausedList.length > 0"
    variant="quiet"
    size="sm"
    data-testid="paused-agents"
    :title="description"
    :aria-label="description"
    @press="panelTab.openOn('agora')"
  >
    {{ PAUSED_BADGE(control.pausedList.length) }}
  </UiButton>
</template>
