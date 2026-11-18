<script setup lang="ts">
import { computed, ref } from 'vue'
import UiSelect from '../../ui/UiSelect.vue'
import { placeAgent } from '../orchestration/orchestrationApi'
import type { Agent } from '../team/events'
import type { AgentProfile } from '../team/teamApi'
import MapDialog from './MapDialog.vue'
import { AGENT_PICK_LABEL, PLACE_AGENT_TITLE, PLACE_LABEL, PLACE_PLACEHOLDER } from './teamMapLayers'

const props = defineProps<{ projectId: number; teamId: string; layerId: string; agents: AgentProfile[] }>()
const emit = defineEmits<{ close: [] }>()

const DEFAULT_MAX_INSTANCES = 1

const agentOptions = computed(() => props.agents.map((agent) => ({ label: agent.name, value: agent.id })))
const agentId = ref('')
const submitting = ref(false)
const failure = ref<string | null>(null)
const canSubmit = computed(() => agentId.value !== '')

async function submit(): Promise<void> {
  if (!canSubmit.value || submitting.value) return
  submitting.value = true
  failure.value = null
  try {
    await placeAgent(props.projectId, {
      agentId: agentId.value as Agent,
      teamId: props.teamId,
      layerId: props.layerId,
      instructions: '',
      maxInstances: DEFAULT_MAX_INSTANCES,
    })
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
    :title="PLACE_AGENT_TITLE"
    :submit-label="PLACE_LABEL"
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
