<script setup lang="ts">
import { computed, ref } from 'vue'
import { useChannels } from './useChannels'
import type { Channel } from './channelsApi'
import type { Agent } from '../team/events'
import { useRoster } from '../team/useRoster'
import AgentPortrait from '../team/AgentPortrait.vue'
import { PORTRAIT_SIZES } from '../team/portraitArt'
import UiButton from '../../ui/UiButton.vue'
import UiNotice from '../../ui/UiNotice.vue'

const MIN_MEMBERS = 2
const CREATE_LABEL = 'Criar canal'
const SAVE_LABEL = 'Salvar canal'
const FORM_CLASSES = 'flex flex-col gap-3 rounded-control border border-line bg-surface-2 p-3'
const INPUT_CLASSES =
  'h-8.5 w-full rounded-control bg-surface px-3 text-label text-ink placeholder:text-muted focus:outline-2 focus:outline-solid focus:outline-accent'
const MEMBER_CLASSES =
  'flex cursor-pointer items-center gap-1.5 rounded-full border border-line bg-surface py-1 pl-1 pr-3 text-caption text-ink has-[:checked]:border-accent has-[:checked]:bg-accent-soft has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-solid has-[:focus-visible]:outline-accent'

const props = defineProps<{ channel?: Channel }>()
const emit = defineEmits<{ done: [] }>()

const channels = useChannels()
const roster = useRoster()
const name = ref(props.channel?.name ?? '')
const members = ref<Agent[]>([...(props.channel?.members ?? [])])
const busy = ref(false)
const failure = ref<string | null>(null)

const canSubmit = computed(() => name.value.trim() !== '' && members.value.length >= MIN_MEMBERS && !busy.value)

async function submit(): Promise<void> {
  failure.value = null
  busy.value = true
  try {
    if (props.channel) {
      await channels.update(props.channel.id, name.value, members.value)
    } else {
      await channels.create(name.value, members.value)
    }
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
    <fieldset class="flex flex-col gap-2">
      <legend class="mb-1 text-caption text-muted">Agentes (2 ou mais)</legend>
      <div class="flex flex-wrap gap-2">
        <label v-for="agent in roster.agents" :key="agent.id" :class="MEMBER_CLASSES">
          <input v-model="members" type="checkbox" class="sr-only" :value="agent.id" />
          <AgentPortrait :who="agent.id" :size="PORTRAIT_SIZES.stack" />
          {{ agent.name }}
        </label>
      </div>
    </fieldset>
    <UiNotice v-if="failure" :text="failure" @close="failure = null" />
    <UiButton variant="approve" size="sm" :disabled="!canSubmit" @press="submit">{{ channel ? SAVE_LABEL : CREATE_LABEL }}</UiButton>
  </div>
</template>
