<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import type { AgentProfile } from '../team/teamApi'
import type { AgentState, TurnActivity } from '../team/events'
import { activityRows, formatClock, type ActivityRowModel, type AgentNameOf, type CardTitleOf } from '../team/activitySummary'
import ActivityRow from '../team/ActivityRow.vue'
import MarkdownText from '../team/MarkdownText.vue'
import TurnPlanCard from '../team/TurnPlanCard.vue'
import { useBacklog } from '../backlog/useBacklog'
import { useRoster } from '../team/useRoster'
import { useTurnPlans } from '../team/useTurnPlans'
import UiHandLine from '../../ui/UiHandLine.vue'
import UiIcon from '../../ui/UiIcon.vue'
import { useProjects } from '../projects/useProjects'
import { stopAgentTurn } from '../team/teamApi'
import { useFeed } from '../team/useFeed'
import { liveTimeline } from './liveTimeline'
import { useConsole } from './useConsole'

const props = defineProps<{ agent: AgentProfile; state: AgentState; since?: number; activity?: TurnActivity; itemId?: string }>()

const SENDING_LABEL = 'Enviando…'
const THINKING_LABEL = 'Pensando…'
const STOP_LABEL = 'Parar'
const STOP_ICON_PATH = 'M4.5 4.5h7v7h-7z'
const TICK_MS = 1000
const DOT_CLASSES = 'size-2 fill-current motion-reduce:animate-none'
const DOT_ANIMATIONS = ['animate-dot-1', 'animate-dot-2', 'animate-dot-3']

const consoleStore = useConsole()
const backlog = useBacklog()
const roster = useRoster()
const turnPlans = useTurnPlans()
const projects = useProjects()
const feed = useFeed()
const stopping = ref(false)
const now = ref(Date.now())

async function stopAgent(): Promise<void> {
  const projectId = projects.selected?.id
  if (stopping.value || projectId === undefined) return
  stopping.value = true
  try {
    await stopAgentTurn(projectId, props.agent.id, props.itemId)
  } catch (error) {
    feed.feed.push({ type: 'error', message: error instanceof Error ? error.message : String(error) })
  } finally {
    stopping.value = false
  }
}
let timer: number | undefined

onMounted(() => {
  timer = window.setInterval(() => {
    now.value = Date.now()
  }, TICK_MS)
})
onUnmounted(() => clearInterval(timer))

type LiveSegment = { kind: 'text'; text: string } | { kind: 'tools'; rows: ActivityRowModel[] }

const cardTitleOf: CardTitleOf = (cardId) => backlog.items.find((item) => item.id === cardId)?.title
const agentNameOf: AgentNameOf = (id) => roster.profileOf(id)?.name

// Fora de 'thinking' (envio) o turno ainda não começou: sem isso, currentTurn devolve a resposta anterior.
const timelineSegments = computed<LiveSegment[]>(() => {
  if (props.state !== 'thinking') {
    return []
  }
  return liveTimeline(consoleStore.entriesOf(props.agent.id), props.activity?.tools ?? []).map((segment) =>
    segment.kind === 'text'
      ? segment
      : { kind: 'tools', rows: activityRows(segment.tools, cardTitleOf, agentNameOf) },
  )
})

const thinkingPreview = computed(() =>
  props.state === 'thinking' ? props.activity?.thinkingText?.trim() || undefined : undefined,
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
  <div class="group flex min-w-0 max-w-xl flex-col gap-1">
    <span class="flex min-w-0 items-center gap-2 text-caption text-muted">
      <span class="min-w-0 truncate" :title="agent.name">{{ agent.name }}</span>
      <button
        type="button"
        class="inline-flex shrink-0 cursor-pointer items-center gap-1 rounded-sm border border-line bg-surface px-1.5 py-0.5 text-tiny font-medium text-danger opacity-0 transition-opacity duration-150 hover:bg-danger-soft focus-visible:opacity-100 group-hover:opacity-100 disabled:opacity-40 motion-reduce:transition-none"
        :disabled="stopping"
        :title="`Parar ${agent.name}`"
        :aria-label="`Parar ${agent.name}`"
        data-testid="agent-thinking-stop"
        @click.stop="void stopAgent()"
      >
        <UiIcon :path="STOP_ICON_PATH" :size="10" />
        <span>{{ STOP_LABEL }}</span>
      </button>
    </span>
    <TurnPlanCard v-if="activePlan" :plan="activePlan" status="active" />
    <template v-for="(segment, index) in timelineSegments" :key="index">
      <div v-if="segment.kind === 'text'" class="line-clamp-4 min-w-0">
        <MarkdownText :text="segment.text" />
      </div>
      <ul
        v-else-if="segment.rows.length > 0"
        class="mt-0.5 flex min-w-0 max-w-full flex-col gap-1 text-caption text-muted"
        data-testid="activity-live"
      >
        <ActivityRow v-for="row in segment.rows" :key="row.id" :row="row" />
      </ul>
    </template>
    <div v-if="thinkingPreview" class="line-clamp-4 min-w-0">
      <MarkdownText :text="thinkingPreview" />
    </div>
    <span class="flex min-w-0 items-center gap-2 text-caption text-muted">
      <span class="min-w-0 truncate" :title="`${agent.name} · ${label}`" data-testid="agent-thinking-label">
        {{ label }}
      </span>
      <span class="flex shrink-0 items-center gap-1" aria-hidden="true">
        <UiHandLine v-for="animation in DOT_ANIMATIONS" :key="animation" variant="circle" :class="[DOT_CLASSES, animation]" />
      </span>
    </span>
  </div>
</template>
