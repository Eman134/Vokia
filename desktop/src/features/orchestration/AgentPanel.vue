<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiSelect from '../../ui/UiSelect.vue'
import { LAYER_LABEL, TEAM_LABEL } from '../agents/agentFormCopy'
import AgentPortrait from '../team/AgentPortrait.vue'
import type { AgentProfile } from '../team/teamApi'
import CountStepper from './CountStepper.vue'
import { VIEW_COPY } from './orchestrationLabels'
import type { OrchLayer, OrchPlacement, OrchTeam } from './orchestrationTypes'
import { useOrchestration } from './useOrchestration'

const INSTRUCTIONS_MAX_CHARS = 4000
const INSTANCES_MIN = 1
const INSTANCES_MAX = 4
const DEFAULT_INSTANCES = 1
const PORTRAIT_SIZE = 36
const TEXTAREA_CLASSES =
  'w-full min-w-0 rounded-control border border-line bg-surface px-3 py-2 text-control text-ink placeholder:text-faint focus:outline-2 focus:outline-solid focus:outline-accent'

const props = defineProps<{
  agent: AgentProfile
  placement: OrchPlacement | undefined
  teams: OrchTeam[]
  layers: OrchLayer[]
}>()
const emit = defineEmits<{ close: [] }>()

const orchestration = useOrchestration()

const teamOptions = computed(() => props.teams.map((team) => ({ label: team.name, value: team.id })))
const layerOptions = computed(() => props.layers.map((layer) => ({ label: layer.name, value: layer.id })))
const canPlace = computed(() => props.teams.length > 0 && props.layers.length > 0)

const teamId = ref(props.placement?.teamId ?? props.teams[0]?.id ?? '')
const layerId = ref(props.placement?.layerId ?? props.layers[0]?.id ?? '')
const instructions = ref(props.placement?.instructions ?? '')
const maxInstances = ref(props.placement?.maxInstances ?? DEFAULT_INSTANCES)

const instructionsCount = computed(() => `${instructions.value.length} / ${INSTRUCTIONS_MAX_CHARS}`)
const canSave = computed(() => canPlace.value && teamId.value !== '' && layerId.value !== '')

function save(): void {
  if (!canSave.value) return
  void orchestration.placeAgent({
    agentId: props.agent.id,
    teamId: teamId.value,
    layerId: layerId.value,
    instructions: instructions.value,
    maxInstances: maxInstances.value,
  })
}
</script>

<template>
  <section class="flex min-w-0 flex-col gap-4 rounded-[24px] border border-line bg-surface p-4.5">
    <header class="flex items-center justify-between gap-3">
      <div class="flex min-w-0 items-center gap-2.5">
        <AgentPortrait :who="agent.id" :size="PORTRAIT_SIZE" />
        <h3 class="truncate text-control font-bold text-ink">{{ agent.name }}</h3>
      </div>
      <UiButton variant="quiet" size="sm" @press="emit('close')">{{ VIEW_COPY.close }}</UiButton>
    </header>

    <p v-if="!canPlace" class="text-body text-muted">{{ VIEW_COPY.noPlacementPossible }}</p>
    <template v-else>
      <label class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ TEAM_LABEL }}</span>
        <UiSelect v-model="teamId" :options="teamOptions" :label="TEAM_LABEL" />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ LAYER_LABEL }}</span>
        <UiSelect v-model="layerId" :options="layerOptions" :label="LAYER_LABEL" />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="flex items-baseline justify-between gap-3 text-label font-medium text-muted">
          {{ VIEW_COPY.instructions }}
          <span class="font-mono text-tiny font-normal text-faint">{{ instructionsCount }}</span>
        </span>
        <textarea v-model="instructions" rows="4" :class="TEXTAREA_CLASSES" />
      </label>
      <div class="flex flex-col gap-1.5">
        <span class="text-label font-medium text-muted">{{ VIEW_COPY.instances }}</span>
        <CountStepper v-model="maxInstances" :label="VIEW_COPY.instances" :min="INSTANCES_MIN" :max="INSTANCES_MAX" />
      </div>
      <div class="flex justify-end">
        <UiButton variant="primary" size="lg" :disabled="!canSave" @press="save()">{{ VIEW_COPY.saveAgent }}</UiButton>
      </div>
    </template>
  </section>
</template>
