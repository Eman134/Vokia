<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiField from '../../ui/UiField.vue'
import { CANCEL_LABEL, SAVE_LABEL } from '../agents/agentFormCopy'
import ModalPanel from './ModalPanel.vue'
import { VIEW_COPY } from './orchestrationLabels'

const props = defineProps<{ title: string; initialName: string }>()
const emit = defineEmits<{ save: [name: string]; cancel: [] }>()

const name = ref(props.initialName)
const canSave = computed(() => name.value.trim() !== '')

function save(): void {
  if (canSave.value) emit('save', name.value.trim())
}
</script>

<template>
  <ModalPanel :title="title" @close="emit('cancel')">
    <form class="flex flex-col gap-4" @submit.prevent="save">
      <fieldset class="m-0 flex min-w-0 flex-col gap-4 border-0 p-0">
        <UiField v-model="name" :label="VIEW_COPY.teamName" />
      </fieldset>
      <div class="flex justify-end gap-2">
        <UiButton variant="quiet" size="lg" @press="emit('cancel')">{{ CANCEL_LABEL }}</UiButton>
        <UiButton variant="primary" size="lg" :disabled="!canSave" @press="save">{{ SAVE_LABEL }}</UiButton>
      </div>
    </form>
  </ModalPanel>
</template>
