<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import UiButton from '../../ui/UiButton.vue'
import UiNotice from '../../ui/UiNotice.vue'
import type { AgentProfile } from '../team/teamApi'
import { useRoster } from '../team/useRoster'
import {
  EXTRA_INSTRUCTIONS_HINT,
  EXTRA_INSTRUCTIONS_LABEL,
  EXTRA_INSTRUCTIONS_SAVE_LABEL,
} from './agentesCopy'

const EXTRA_INSTRUCTIONS_MAX_CHARS = 4000
const TEXTAREA_CLASSES =
  'min-h-32 w-full min-w-0 rounded-control border border-line bg-surface px-3 py-2 text-control text-ink placeholder:text-faint focus:outline-2 focus:outline-solid focus:outline-accent'

const props = defineProps<{ agent: AgentProfile }>()

const roster = useRoster()
const draft = ref(props.agent.extraInstructions)
const saving = ref(false)
const failure = ref<string | null>(null)

const length = computed(() => draft.value.trim().length)
const changed = computed(() => draft.value.trim() !== props.agent.extraInstructions)
const canSave = computed(
  () => changed.value && length.value <= EXTRA_INSTRUCTIONS_MAX_CHARS && !saving.value,
)

// O backend devolve o texto já sem espaços nas pontas; o rascunho acompanha o que ficou gravado.
watch(
  () => props.agent.extraInstructions,
  (value) => {
    draft.value = value
  },
)

async function save(): Promise<void> {
  if (!canSave.value) return
  saving.value = true
  failure.value = null
  try {
    await roster.setInstructions(props.agent.id, draft.value)
  } catch (error) {
    failure.value = String(error)
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <div class="flex flex-col gap-2">
    <label class="flex flex-col gap-1.5">
      <span class="text-label font-medium text-muted">{{ EXTRA_INSTRUCTIONS_LABEL }}</span>
      <textarea
        v-model="draft"
        :class="TEXTAREA_CLASSES"
        :maxlength="EXTRA_INSTRUCTIONS_MAX_CHARS"
        rows="5"
        :disabled="saving"
      />
    </label>
    <p class="text-caption text-muted">{{ EXTRA_INSTRUCTIONS_HINT }}</p>
    <p class="text-caption text-muted" data-testid="extra-instructions-count">
      {{ length }}/{{ EXTRA_INSTRUCTIONS_MAX_CHARS }}
    </p>
    <UiNotice v-if="failure !== null" :text="failure" @close="failure = null" />
    <div class="flex justify-end">
      <UiButton variant="primary" size="sm" :disabled="!canSave" @press="save()">
        {{ EXTRA_INSTRUCTIONS_SAVE_LABEL }}
      </UiButton>
    </div>
  </div>
</template>
