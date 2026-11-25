<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import type { AgentProfile } from '../team/teamApi'
import type { AgentState, TurnActivity } from '../team/events'
import { formatClock } from '../team/activitySummary'
import ActivityRun from '../team/ActivityRun.vue'
import MarkdownText from '../team/MarkdownText.vue'
import TurnPlanCard from '../team/TurnPlanCard.vue'
import { useTurnPlans } from '../team/useTurnPlans'
import UiHandLine from '../../ui/UiHandLine.vue'
import { currentTurn } from './consoleTurn'
import { useConsole } from './useConsole'

const props = defineProps<{ agent: AgentProfile; state: AgentState; since?: number; activity?: TurnActivity; itemId?: string }>()

const SENDING_LABEL = 'Enviando…'
const THINKING_LABEL = 'Pensando…'
const TICK_MS = 1000
const DOT_CLASSES = 'size-2 fill-current motion-reduce:animate-none'
const DOT_ANIMATIONS = ['animate-dot-1', 'animate-dot-2', 'animate-dot-3']

const consoleStore = useConsole()
const turnPlans = useTurnPlans()
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
const stepLabel = computed(() =>
  props.state === 'thinking' ? props.activity?.currentStepLabel?.trim() || undefined : undefined,
)
const activePlan = computed(() => {
  if (props.state !== 'thinking') return undefined
  const live = turnPlans.livePlanOf(props.agent.id, props.itemId)?.plan
  if (live && live.steps.length > 0) return live
  const fromActivity = props.activity?.plan
  return fromActivity && fromActivity.steps.length > 0 ? fromActivity : undefined
})

const label = computed(() => {
  if (props.state !== 'thinking') {
    return SENDING_LABEL
  }
  const base = stepLabel.value ?? THINKING_LABEL
  if (props.since === undefined) {
    return base
  }
  const clock = formatClock(Math.max(0, now.value - props.since))
  return stepLabel.value ? `${base} · ${clock}` : `${base} ${clock}`
})
</script>

<template>
  <div class="flex min-w-0 max-w-xl flex-col gap-1">
    <span class="flex min-w-0 items-center gap-2 text-caption text-muted">
      <span class="min-w-0 truncate" :title="`${agent.name} · ${label}`" data-testid="agent-thinking-label">
        {{ agent.name }} · {{ label }}
      </span>
      <span class="flex shrink-0 items-center gap-1" aria-hidden="true">
        <UiHandLine v-for="animation in DOT_ANIMATIONS" :key="animation" variant="circle" :class="[DOT_CLASSES, animation]" />
      </span>
    </span>
    <TurnPlanCard v-if="activePlan" :plan="activePlan" status="active" />
    <ActivityRun v-if="activity" :activity="activity" live />
    <div v-if="preview" class="line-clamp-4 min-w-0">
      <MarkdownText :text="preview" />
    </div>
  </div>
</template>
