<script setup lang="ts">
import { computed, ref } from 'vue'
import UiField from '../../ui/UiField.vue'
import UiSelect from '../../ui/UiSelect.vue'
import { CREATE_LABEL, NAME_LABEL } from '../agents/agentFormCopy'
import { LAYER_KIND_LABELS } from '../orchestration/orchestrationLabels'
import { saveLayer } from '../orchestration/orchestrationApi'
import { LAYER_KIND_VALUES, type LayerKind, type OrchLayer } from '../orchestration/orchestrationTypes'
import MapDialog from './MapDialog.vue'
import { DESCRIPTION_LABEL, NEW_LAYER_TITLE, PLACE_PLACEHOLDER, TYPE_LABEL } from './teamMapLayers'

const props = defineProps<{ projectId: number }>()
const emit = defineEmits<{ created: [layer: OrchLayer]; close: [] }>()

const kindOptions = LAYER_KIND_VALUES.map((kind) => ({ label: LAYER_KIND_LABELS[kind], value: kind }))
const name = ref('')
const description = ref('')
const kind = ref('')
const submitting = ref(false)
const failure = ref<string | null>(null)
const canSubmit = computed(() => name.value.trim() !== '' && kind.value !== '')

async function submit(): Promise<void> {
  if (!canSubmit.value || submitting.value) return
  submitting.value = true
  failure.value = null
  try {
    const layer = await saveLayer(props.projectId, {
      id: null,
      name: name.value.trim(),
      description: description.value.trim(),
      kind: kind.value as LayerKind,
      instructions: '',
    })
    submitting.value = false
    emit('created', layer)
    emit('close')
  } catch (error) {
    failure.value = String(error)
    submitting.value = false
  }
}
</script>

<template>
  <MapDialog
    :title="NEW_LAYER_TITLE"
    :submit-label="CREATE_LABEL"
    :can-submit="canSubmit"
    :submitting="submitting"
    :failure="failure"
    @submit="submit()"
    @close="emit('close')"
    @dismiss-failure="failure = null"
  >
    <UiField v-model="name" :label="NAME_LABEL" />
    <UiField v-model="description" :label="DESCRIPTION_LABEL" />
    <label class="flex flex-col gap-1.5">
      <span class="text-label font-medium text-muted">{{ TYPE_LABEL }}</span>
      <UiSelect v-model="kind" :options="kindOptions" :label="TYPE_LABEL" :placeholder="PLACE_PLACEHOLDER" />
    </label>
  </MapDialog>
</template>
