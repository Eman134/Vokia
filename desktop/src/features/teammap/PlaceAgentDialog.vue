<script setup lang="ts">
import { computed, ref } from 'vue'
import UiSelect from '../../ui/UiSelect.vue'
import { LAYER_LABEL, TEAM_LABEL } from '../agents/agentFormCopy'
import { placeAgent } from '../orchestration/orchestrationApi'
import { useOrchestration } from '../orchestration/useOrchestration'
import type { AgentProfile } from '../team/teamApi'
import MapDialog from './MapDialog.vue'
import { PLACE_AGENT_TITLE, PLACE_LABEL, PLACE_PLACEHOLDER } from './teamMapLayers'

const props = defineProps<{ projectId: number; agent: AgentProfile }>()
const emit = defineEmits<{ placed: []; close: [] }>()

const orchestration = useOrchestration()
const teamOptions = computed(() =>
  (orchestration.snapshot?.teams ?? []).map((team) => ({ label: team.name, value: team.id })),
)
const layerOptions = computed(() =>
  (orchestration.snapshot?.layers ?? []).map((layer) => ({ label: layer.name, value: layer.id })),
)
const teamId = ref('')
const layerId = ref('')
const submitting = ref(false)
const failure = ref<string | null>(null)
const canSubmit = computed(() => teamId.value !== '' && layerId.value !== '')

async function submit(): Promise<void> {
  if (!canSubmit.value || submitting.value) return
  submitting.value = true
  failure.value = null
  try {
    await placeAgent(props.projectId, {
      agentId: props.agent.id,
      teamId: teamId.value,
      layerId: layerId.value,
      instructions: '',
      maxInstances: 1,
    })
    submitting.value = false
    emit('placed')
    emit('close')
  } catch (error) {
    failure.value = String(error)
    submitting.value = false
  }
}
</script>

<template>
  <MapDialog
    :title="`${PLACE_AGENT_TITLE} · ${agent.name}`"
    :submit-label="PLACE_LABEL"
    :can-submit="canSubmit"
    :submitting="submitting"
    :failure="failure"
    @submit="submit()"
    @close="emit('close')"
    @dismiss-failure="failure = null"
  >
    <label class="flex flex-col gap-1.5">
      <span class="text-label font-medium text-muted">{{ TEAM_LABEL }}</span>
      <UiSelect v-model="teamId" :options="teamOptions" :label="TEAM_LABEL" :placeholder="PLACE_PLACEHOLDER" />
    </label>
    <label class="flex flex-col gap-1.5">
      <span class="text-label font-medium text-muted">{{ LAYER_LABEL }}</span>
      <UiSelect v-model="layerId" :options="layerOptions" :label="LAYER_LABEL" :placeholder="PLACE_PLACEHOLDER" />
    </label>
  </MapDialog>
</template>
