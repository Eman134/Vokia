<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import type { AgentProfile } from '../team/teamApi'
import { STATE_LABELS } from '../team/agentStateStyle'
import ConsoleLines from './ConsoleLines.vue'
import { useConsole } from './useConsole'
import { useConversationTab } from '../team/conversationTabs'
import { CONSOLE_PREVIEW_LINES, currentTurn, elapsedLabel } from './consoleTurn'

const props = defineProps<{ agent: AgentProfile }>()

const EXPAND_LABEL = 'Expandir'
const WAITING_OUTPUT_TEXT = 'aguardando a primeira saída…'
const CLOCK_TICK_MS = 1000

const consoleStore = useConsole()
const conversationTab = useConversationTab()
const now = ref(Date.now())
let clock: ReturnType<typeof setInterval> | undefined

const turn = computed(() => currentTurn(consoleStore.entriesOf(props.agent.id)))
const preview = computed(() => turn.value.filter((entry) => entry.kind !== 'start').slice(-CONSOLE_PREVIEW_LINES))
const elapsed = computed(() => {
  const first = turn.value[0]
  return first === undefined ? null : elapsedLabel(first.time, now.value)
})

onMounted(() => {
  clock = setInterval(() => {
    now.value = Date.now()
  }, CLOCK_TICK_MS)
})
onUnmounted(() => clearInterval(clock))
</script>

<template>
  <article class="w-full max-w-xl min-w-0 overflow-hidden rounded-card border border-line bg-term-bg">
    <button
      type="button"
      class="flex w-full cursor-pointer items-center gap-2 border-b border-term-line px-3 py-2 text-left focus:outline-2 focus:outline-solid focus:outline-accent"
      :aria-label="`Abrir console de ${agent.name}`"
      @click="conversationTab.showTerminal(agent.id)"
    >
      <span class="text-caption font-semibold text-term-ink">{{ agent.name }}</span>
      <span class="text-caption text-term-muted">{{ STATE_LABELS.thinking }}</span>
      <span class="flex items-center gap-1" aria-hidden="true">
        <span class="size-1.5 rounded-full bg-term-muted motion-reduce:animate-none animate-dot-1" />
        <span class="size-1.5 rounded-full bg-term-muted motion-reduce:animate-none animate-dot-2" />
        <span class="size-1.5 rounded-full bg-term-muted motion-reduce:animate-none animate-dot-3" />
      </span>
      <span v-if="elapsed" class="font-mono text-caption text-term-muted">{{ elapsed }}</span>
      <span class="ml-auto text-caption font-medium text-term-accent">{{ EXPAND_LABEL }}</span>
    </button>
    <div class="px-3 py-2">
      <ConsoleLines v-if="preview.length > 0" :entries="preview" />
      <p v-else class="font-mono text-small text-term-muted">{{ WAITING_OUTPUT_TEXT }}</p>
    </div>
  </article>
</template>
