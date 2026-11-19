<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import { PAGE_INSET_CLASSES } from '../../ui/pageLayout'
import UiPageHeader from '../../ui/UiPageHeader.vue'
import OrchestrationView from '../orchestration/OrchestrationView.vue'
import TeamsList from '../orchestration/TeamsList.vue'
import { VIEW_COPY } from '../orchestration/orchestrationLabels'
import { useOrchestration } from '../orchestration/useOrchestration'
import { MAP_TITLE } from './teamMapLayers'

const PAGE_CLASSES = `flex min-h-0 min-w-0 flex-1 flex-col gap-5 overflow-hidden ${PAGE_INSET_CLASSES}`

const orchestration = useOrchestration()
const selectedTeamId = ref<string | null>(null)
const selectedTeam = computed(
  () => orchestration.snapshot?.teams.find((team) => team.id === selectedTeamId.value) ?? null,
)
</script>

<template>
  <section :class="PAGE_CLASSES">
    <UiPageHeader :title="selectedTeam ? MAP_TITLE : VIEW_COPY.teamsTitle" />
    <UiEmpty v-if="orchestration.snapshot === null" :title="VIEW_COPY.noProjectTeams" />
    <template v-else-if="selectedTeam">
      <div class="flex min-w-0 items-center gap-3">
        <UiButton variant="quiet" size="sm" @press="selectedTeamId = null">{{ VIEW_COPY.backToTeams }}</UiButton>
        <h2 class="min-w-0 truncate text-control font-bold text-ink">{{ selectedTeam.name }}</h2>
      </div>
      <OrchestrationView :key="selectedTeam.id" :team-id="selectedTeam.id" />
    </template>
    <TeamsList v-else @open="selectedTeamId = $event" />
  </section>
</template>
