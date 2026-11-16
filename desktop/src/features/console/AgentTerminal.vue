<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { agentName } from '../team/agents'
import { useFeed } from '../team/useFeed'
import { useProjects } from '../projects/useProjects'
import UiButton from '../../ui/UiButton.vue'
import UiEmpty from '../../ui/UiEmpty.vue'
import UiNotice from '../../ui/UiNotice.vue'
import ConsoleBody from './ConsoleBody.vue'
import type { ConsoleFilter } from './consoleFilters'
import { useConsole } from './useConsole'

const NO_AGENT_TITLE = 'Escolha um agente na lista para ver o terminal dele.'
const ANSWER_LABEL = 'Responder em Conversa'
const TERMINAL_LABEL_PREFIX = 'Terminal de '

const emit = defineEmits<{ answer: [] }>()
const feed = useFeed()
const consoleStore = useConsole()
const projects = useProjects()
const filter = ref<ConsoleFilter>('all')

const agent = computed(() => (feed.screen.kind === 'agent' ? feed.screen.agent : null))
const entries = computed(() => (agent.value === null ? [] : consoleStore.entriesOf(agent.value)))
const loadError = computed(() => (agent.value === null ? undefined : consoleStore.loadErrors[agent.value]))
const waiting = computed(() => agent.value !== null && feed.permissions.some((permission) => permission.agent === agent.value))

watch(
  [agent, () => projects.selected?.id],
  () => {
    filter.value = 'all'
    if (agent.value !== null) void consoleStore.load(agent.value)
  },
  { immediate: true },
)
</script>

<template>
  <section
    v-if="agent !== null"
    :aria-label="TERMINAL_LABEL_PREFIX + agentName(agent)"
    class="flex min-h-0 flex-1 flex-col overflow-hidden"
  >
    <div v-if="waiting" class="flex items-center justify-between gap-3 border-b border-line px-4.5 py-2">
      <span class="text-caption text-muted">{{ agentName(agent) }} espera sua permissão.</span>
      <UiButton variant="primary" size="sm" @press="emit('answer')">{{ ANSWER_LABEL }}</UiButton>
    </div>
    <ConsoleBody v-model:filter="filter" :entries="entries">
      <UiNotice v-if="loadError" class="mx-4 mb-2" :text="loadError" @close="consoleStore.dismissError(agent)" />
    </ConsoleBody>
  </section>
  <UiEmpty v-else class="m-auto" :title="NO_AGENT_TITLE" />
</template>
