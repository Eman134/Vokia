<script setup lang="ts">
import { computed, ref } from 'vue'
import UiSelect from '../../ui/UiSelect.vue'
import { placeAgent, unplaceAgent } from '../orchestration/orchestrationApi'
import { useOrchestration } from '../orchestration/useOrchestration'
import type { Agent } from '../team/events'
import type { AgentProfile } from '../team/teamApi'
import MapDialog from './MapDialog.vue'
import {
  AGENT_PICK_LABEL,
  LINK_AGENT_TITLE,
  LINK_LABEL,
  PLACE_PLACEHOLDER,
  REPLACE_AGENT_TITLE,
  REPLACE_CONFIRM_LABEL,
} from './teamMapLayers'

const props = defineProps<{
  projectId: number
  teamId: string
  layerId: string
  agents: AgentProfile[]
  replacingId: Agent | null
}>()
const emit = defineEmits<{ close: [] }>()

const DEFAULT_MAX_INSTANCES = 1
const PLACED_HINT_SEPARATOR = ' · '

const orchestration = useOrchestration()

function placementOf(id: string) {
  return orchestration.snapshot?.placements.find((candidate) => candidate.agentId === id)
}

const choices = computed(() => props.agents.filter((agent) => agent.id !== props.replacingId))

const agentOptions = computed(() =>
  choices.value.map((agent) => {
    const placement = placementOf(agent.id)
    if (!placement) return { label: agent.name, value: agent.id }
    const team = orchestration.snapshot?.teams.find((candidate) => candidate.id === placement.teamId)?.name ?? ''
    const layer = orchestration.snapshot?.layers.find((candidate) => candidate.id === placement.layerId)?.name ?? ''
    return { label: [agent.name, team, layer].join(PLACED_HINT_SEPARATOR), value: agent.id }
  }),
)

const agentId = ref('')
const submitting = ref(false)
const failure = ref<string | null>(null)
const canSubmit = computed(() => agentId.value !== '')

// Troca: o novo agente herda a posição do substituído (instruções e réplicas); o substituído sai depois.
async function replace(replacedId: Agent): Promise<void> {
  const replaced = placementOf(replacedId)
  await placeAgent(props.projectId, {
    agentId: agentId.value as Agent,
    teamId: props.teamId,
    layerId: props.layerId,
    instructions: replaced?.instructions ?? '',
    maxInstances: replaced?.maxInstances ?? DEFAULT_MAX_INSTANCES,
  })
  await unplaceAgent(props.projectId, replacedId)
}

// Vincular ou mover: só time e camada mudam; instruções e réplicas da posição atual são mantidas.
async function link(): Promise<void> {
  const current = placementOf(agentId.value)
  await placeAgent(props.projectId, {
    agentId: agentId.value as Agent,
    teamId: props.teamId,
    layerId: props.layerId,
    instructions: current?.instructions ?? '',
    maxInstances: current?.maxInstances ?? DEFAULT_MAX_INSTANCES,
  })
}

async function submit(): Promise<void> {
  if (!canSubmit.value || submitting.value) return
  submitting.value = true
  failure.value = null
  try {
    if (props.replacingId !== null) await replace(props.replacingId)
    else await link()
    submitting.value = false
    emit('close')
  } catch (error) {
    failure.value = String(error)
    submitting.value = false
  }
}
</script>

<template>
  <MapDialog
    :title="replacingId ? REPLACE_AGENT_TITLE : LINK_AGENT_TITLE"
    :submit-label="replacingId ? REPLACE_CONFIRM_LABEL : LINK_LABEL"
    :can-submit="canSubmit"
    :submitting="submitting"
    :failure="failure"
    @submit="submit()"
    @close="emit('close')"
    @dismiss-failure="failure = null"
  >
    <label class="flex flex-col gap-1.5">
      <span class="text-label font-medium text-muted">{{ AGENT_PICK_LABEL }}</span>
      <UiSelect
        :model-value="agentId"
        :options="agentOptions"
        :label="AGENT_PICK_LABEL"
        :placeholder="PLACE_PLACEHOLDER"
        @update:model-value="agentId = $event ?? ''"
      />
    </label>
  </MapDialog>
</template>
