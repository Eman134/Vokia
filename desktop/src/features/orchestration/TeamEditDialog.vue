<script setup lang="ts">
import { computed, ref } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiField from '../../ui/UiField.vue'
import UiSelect from '../../ui/UiSelect.vue'
import { CANCEL_LABEL, SAVE_LABEL } from '../agents/agentFormCopy'
import type { Agent } from '../team/events'
import { useRoster } from '../team/useRoster'
import ModalPanel from './ModalPanel.vue'
import { INSTRUCTIONS_MAX_CHARS, TEXTAREA_CLASSES } from './orchestrationForm'
import { VIEW_COPY } from './orchestrationLabels'

const AUTOMATIC_LEADER = ''

export interface TeamEdit {
  name: string
  instructions: string
  leaderAgent: Agent | null
}

const props = defineProps<{
  title: string
  initialName: string
  initialInstructions: string
  initialLeader: Agent | null
  // Agentes posicionados no time: só eles podem liderar. Vazio (time novo) esconde a escolha de líder.
  members: readonly Agent[]
}>()
const emit = defineEmits<{ save: [edit: TeamEdit]; cancel: [] }>()

const roster = useRoster()
const name = ref(props.initialName)
const instructions = ref(props.initialInstructions)
const leader = ref<string>(props.initialLeader ?? AUTOMATIC_LEADER)

const leaderOptions = computed(() => [
  { label: VIEW_COPY.leaderAutomatic, value: AUTOMATIC_LEADER },
  ...props.members.map((agent) => ({ label: roster.nameOf(agent), value: agent })),
])
const instructionsCount = computed(() => `${instructions.value.length} / ${INSTRUCTIONS_MAX_CHARS}`)
const canSave = computed(() => name.value.trim() !== '' && instructions.value.length <= INSTRUCTIONS_MAX_CHARS)

function save(): void {
  if (!canSave.value) return
  emit('save', {
    name: name.value.trim(),
    instructions: instructions.value,
    leaderAgent: leader.value === AUTOMATIC_LEADER ? null : (leader.value as Agent),
  })
}
</script>

<template>
  <ModalPanel :title="title" @close="emit('cancel')">
    <form class="flex flex-col gap-4" @submit.prevent="save">
      <fieldset class="m-0 flex min-w-0 flex-col gap-4 border-0 p-0">
        <UiField v-model="name" :label="VIEW_COPY.teamName" />
        <div v-if="members.length > 0" class="flex flex-col gap-1.5">
          <span class="text-label font-medium text-muted">{{ VIEW_COPY.leaderBadge }}</span>
          <UiSelect v-model="leader" :options="leaderOptions" :label="VIEW_COPY.leaderBadge" />
        </div>
        <label class="flex flex-col gap-1.5">
          <span class="flex items-baseline justify-between gap-3 text-label font-medium text-muted">
            {{ VIEW_COPY.instructions }}
            <span class="font-mono text-tiny font-normal text-faint">{{ instructionsCount }}</span>
          </span>
          <textarea v-model="instructions" rows="6" :class="TEXTAREA_CLASSES" />
        </label>
      </fieldset>
      <div class="flex justify-end gap-2">
        <UiButton variant="quiet" size="lg" @press="emit('cancel')">{{ CANCEL_LABEL }}</UiButton>
        <UiButton variant="primary" size="lg" :disabled="!canSave" @press="save">{{ SAVE_LABEL }}</UiButton>
      </div>
    </form>
  </ModalPanel>
</template>
