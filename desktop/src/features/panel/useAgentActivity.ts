import { computed } from 'vue'
import { useDecisions } from '../decisions/useDecisions'
import { STATE_LABELS } from '../team/agentStateStyle'
import type { Agent, VokiaMessage } from '../team/events'
import { useTeam } from '../team/useTeam'
import { PAUSED_STEP, type AgentStatus } from './nowCopy'
import { useAgentControl } from './useAgentControl'

export const RECENT_ACTIONS_LIMIT = 8
const RECENT_TEXT_LIMIT = 80
const ELLIPSIS = '…'

export interface RecentAction {
  time: number
  text: string
}

function truncate(text: string): string {
  return text.length > RECENT_TEXT_LIMIT ? `${text.slice(0, RECENT_TEXT_LIMIT)}${ELLIPSIS}` : text
}

function isToday(time: number): boolean {
  return new Date(time).toDateString() === new Date().toDateString()
}

export function useAgentActivity(agent: Agent) {
  const team = useTeam()
  const control = useAgentControl()
  const decisions = useDecisions()

  const involved = computed(() =>
    team.feed.filter(
      (entry): entry is VokiaMessage => entry.type === 'message' && (entry.from === agent || entry.to === agent),
    ),
  )
  const pendingRequest = computed<VokiaMessage | null>(() => {
    const index = involved.value.map((message) => message.to === agent).lastIndexOf(true)
    if (index === -1 || involved.value.slice(index + 1).some((message) => message.from === agent)) {
      return null
    }
    return involved.value[index] ?? null
  })
  const task = computed(() => pendingRequest.value?.text ?? null)
  const since = computed(() => pendingRequest.value?.time ?? null)
  const recent = computed<RecentAction[]>(() =>
    involved.value
      .slice(-RECENT_ACTIONS_LIMIT)
      .reverse()
      .map((message) => ({ time: message.time, text: truncate(message.text) })),
  )
  const queue = computed(() => team.queuedFor(agent))
  const doneToday = computed(
    () =>
      involved.value.filter((message) => message.from === agent && message.to === 'user' && isToday(message.time))
        .length,
  )
  const status = computed<AgentStatus>(() => {
    if (control.isPaused(agent)) {
      return 'pausado'
    }
    if (decisions.cards.some((card) => card.agent === agent)) {
      return 'precisa de você'
    }
    return team.states[agent] === 'idle' ? 'livre' : 'trabalhando'
  })
  const step = computed(() => (status.value === 'pausado' ? PAUSED_STEP : STATE_LABELS[team.states[agent]]))

  return { task, since, recent, queue, doneToday, status, step }
}
