<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import type { AgentProfile } from '../team/teamApi'
import type { AgentState, TurnActivity } from '../team/events'
import { formatClock } from '../team/activitySummary'
import MarkdownText from '../team/MarkdownText.vue'
import { currentTurn } from './consoleTurn'
import { useConsole } from './useConsole'

const props = defineProps<{ agent: AgentProfile; state: AgentState; since?: number; activity?: TurnActivity }>()

const SENDING_LABEL = 'Enviando…'
const THINKING_LABEL = 'Pensando…'
const TICK_MS = 1000

const consoleStore = useConsole()
const now = ref(Date.now())
let timer: number | undefined

onMounted(() => {
  timer = window.setInterval(() => {
    now.value = Date.now()
  }, TICK_MS)
})
onUnmounted(() => clearInterval(timer))

// Só o que o agente já escreveu neste turno: o stream não traz o texto do raciocínio.
// Fora de 'thinking' (envio) o turno ainda não começou: sem isso, currentTurn devolve a resposta anterior.
const written = computed(() => {
  if (props.state !== 'thinking') {
    return undefined
  }
  return currentTurn(consoleStore.entriesOf(props.agent.id))
    .filter((entry) => entry.kind === 'text')
    .at(-1)?.text
})

const preview = computed(() =>
  props.state === 'thinking' ? props.activity?.thinkingText?.trim() || written.value : undefined,
)
const label = computed(() => {
  if (props.state !== 'thinking') {
    return SENDING_LABEL
  }
  if (props.since === undefined) {
    return THINKING_LABEL
  }
  return `${THINKING_LABEL} ${formatClock(Math.max(0, now.value - props.since))}`
})
</script>

<template>
  <div class="flex min-w-0 max-w-xl flex-col gap-1">
    <span class="flex items-center gap-2 text-caption text-muted">
      {{ agent.name }} · {{ label }}
      <span class="flex items-center gap-1" aria-hidden="true">
        <span class="size-1.5 rounded-full bg-muted motion-reduce:animate-none animate-dot-1" />
        <span class="size-1.5 rounded-full bg-muted motion-reduce:animate-none animate-dot-2" />
        <span class="size-1.5 rounded-full bg-muted motion-reduce:animate-none animate-dot-3" />
      </span>
    </span>
    <div v-if="preview" class="line-clamp-4 min-w-0">
      <MarkdownText :text="preview" />
    </div>
  </div>
</template>
