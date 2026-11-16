<script setup lang="ts">
import { computed, ref } from 'vue'
import { useChannels } from './useChannels'
import type { Agent } from '../team/events'
import { useRoster } from '../team/useRoster'
import UiButton from '../../ui/UiButton.vue'
import UiNotice from '../../ui/UiNotice.vue'

const MIN_MEMBERS = 2
const FORM_CLASSES = 'flex flex-col gap-3 rounded-control border border-line bg-surface-2 p-3'
const INPUT_CLASSES =
  'h-8.5 w-full rounded-control bg-surface px-3 text-label text-ink placeholder:text-muted focus:outline-2 focus:outline-solid focus:outline-accent'
const MEMBER_CLASSES = 'flex items-center gap-2 text-caption text-ink'

const emit = defineEmits<{ done: [] }>()

const channels = useChannels()
const roster = useRoster()
const name = ref('')
const members = ref<Agent[]>([])
const busy = ref(false)
const failure = ref<string | null>(null)

const canSubmit = computed(() => name.value.trim() !== '' && members.value.length >= MIN_MEMBERS && !busy.value)

async function submit(): Promise<void> {
  failure.value = null
  busy.value = true
  try {
    await channels.create(name.value, members.value)
    emit('done')
  } catch (error) {
    failure.value = String(error)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div :class="FORM_CLASSES">
    <input v-model="name" type="text" aria-label="Nome do canal" placeholder="nome-do-canal" :class="INPUT_CLASSES" />
    <fieldset class="flex flex-col gap-1.5">
      <legend class="mb-1 text-caption text-muted">Agentes (2 ou mais)</legend>
      <label v-for="agent in roster.agents" :key="agent.id" :class="MEMBER_CLASSES">
        <input v-model="members" type="checkbox" :value="agent.id" />
        {{ agent.name }}
      </label>
    </fieldset>
    <UiNotice v-if="failure" :text="failure" @close="failure = null" />
    <UiButton variant="approve" size="sm" :disabled="!canSubmit" @press="submit">Criar canal</UiButton>
  </div>
</template>
