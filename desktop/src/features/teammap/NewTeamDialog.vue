<script setup lang="ts">
import { computed, ref } from 'vue'
import UiField from '../../ui/UiField.vue'
import { CREATE_LABEL, NAME_LABEL } from '../agents/agentFormCopy'
import { saveTeam } from '../orchestration/orchestrationApi'
import type { OrchTeam } from '../orchestration/orchestrationTypes'
import MapDialog from './MapDialog.vue'
import { NEW_TEAM_TITLE } from './teamMapLayers'

const props = defineProps<{ projectId: number }>()
const emit = defineEmits<{ created: [team: OrchTeam]; close: [] }>()

const name = ref('')
const submitting = ref(false)
const failure = ref<string | null>(null)
const canSubmit = computed(() => name.value.trim() !== '')

async function submit(): Promise<void> {
  if (!canSubmit.value || submitting.value) return
  submitting.value = true
  failure.value = null
  try {
    const team = await saveTeam(props.projectId, { id: null, name: name.value.trim() })
    submitting.value = false
    emit('created', team)
    emit('close')
  } catch (error) {
    failure.value = String(error)
    submitting.value = false
  }
}
</script>

<template>
  <MapDialog
    :title="NEW_TEAM_TITLE"
    :submit-label="CREATE_LABEL"
    :can-submit="canSubmit"
    :submitting="submitting"
    :failure="failure"
    @submit="submit()"
    @close="emit('close')"
    @dismiss-failure="failure = null"
  >
    <UiField v-model="name" :label="NAME_LABEL" />
  </MapDialog>
</template>
