import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import {
  type Agent,
  type AgentState,
  type CliProblem,
  type VokiaError,
  type VokiaEvent,
  type VokiaMessage,
} from './events'
import {
  checkConnection,
  fetchCliProblem,
  listenConnectionChanges,
  listenTeamEvents,
  sendRequest,
  type UnlistenFn,
} from './teamApi'
import { fetchDbProblem, listProjects, loadHistory } from '../projects/projectsApi'

const CONNECTIVITY_POLL_INTERVAL_MS = 15000
export const DEFAULT_RECIPIENT: Agent = 'iara'

export interface AgentProfile {
  id: Agent
  name: string
  role: string
}

export type FeedEntry = VokiaMessage | VokiaError

export interface QueuedRequest {
  id: number
  text: string
  to: Agent
}

export const AGENTS: readonly AgentProfile[] = [
  { id: 'iara', name: 'Iara', role: 'Tech lead' },
  { id: 'davi', name: 'Davi', role: 'Dev frontend' },
]

export type TimelineItem =
  | { type: 'entry'; start: number; entry: FeedEntry }
  | { type: 'group'; start: number; messages: VokiaMessage[] }

export function isBetweenAgents(message: VokiaMessage): boolean {
  return message.from !== 'user' && message.to !== 'user'
}

export function findNamedAgent(text: string): Agent | null {
  let found: { agent: Agent; index: number } | null = null
  for (const profile of AGENTS) {
    const index = text.search(new RegExp(`(?<!\\p{L})${profile.name}(?!\\p{L})`, 'iu'))
    if (index !== -1 && (found === null || index < found.index)) {
      found = { agent: profile.id, index }
    }
  }
  return found?.agent ?? null
}

export const useTeam = defineStore('team', () => {
  const cliChecked = ref(false)
  const cliProblem = ref<CliProblem | null>(null)
  const startupError = ref<string | null>(null)
  const feed = ref<FeedEntry[]>([])
  const states = ref<Record<Agent, AgentState>>({ iara: 'idle', davi: 'idle' })
  const online = ref(true)
  const checking = ref(false)
  const pendingMention = ref<Agent | null>(null)
  const queue = ref<QueuedRequest[]>([])
  let offlineSignals = 0
  let unlisten: UnlistenFn | null = null
  let stopListeningConnection: (() => void) | null = null
  let pollTimer: ReturnType<typeof setInterval> | null = null
  let pendingCheck: Promise<boolean> | null = null
  let nextQueueId = 0
  let draining = false

  const thinkingAgent = computed<AgentProfile | undefined>(() =>
    AGENTS.find((agent) => states.value[agent.id] === 'thinking'),
  )
  const timeline = computed<TimelineItem[]>(() => {
    const items: TimelineItem[] = []
    feed.value.forEach((entry, index) => {
      const last = items.at(-1)
      if (entry.type !== 'message' || !isBetweenAgents(entry)) {
        items.push({ type: 'entry', start: index, entry })
      } else if (last?.type === 'group') {
        last.messages.push(entry)
      } else {
        items.push({ type: 'group', start: index, messages: [entry] })
      }
    })
    return items
  })

  function refreshConnection(): Promise<boolean> {
    if (pendingCheck === null) {
      const offlineSignalsAtStart = offlineSignals
      pendingCheck = checkConnection()
        .then((reachable) => {
          const stillOnline = reachable && offlineSignalsAtStart === offlineSignals
          online.value = stillOnline
          return stillOnline
        })
        .finally(() => {
          pendingCheck = null
        })
    }
    return pendingCheck
  }

  async function restoreHistory(): Promise<void> {
    try {
      const problem = await fetchDbProblem()
      if (problem !== null) {
        feed.value.push({ type: 'error', message: problem.reason })
        return
      }
      const selected = (await listProjects()).find((project) => project.selected)
      if (selected !== undefined) {
        feed.value = await loadHistory(selected.id)
      }
    } catch (error) {
      feed.value.push({
        type: 'error',
        message: `Não foi possível carregar o histórico: ${String(error)}`,
      })
    }
  }

  async function start(): Promise<void> {
    try {
      cliProblem.value = await fetchCliProblem()
      if (cliProblem.value === null) {
        await restoreHistory()
        unlisten = await listenTeamEvents(apply)
        stopListeningConnection = listenConnectionChanges(
          () => void refreshConnection(),
          () => {
            offlineSignals += 1
            online.value = false
          },
        )
        void refreshConnection()
        pollTimer = setInterval(() => void refreshConnection(), CONNECTIVITY_POLL_INTERVAL_MS)
      }
      cliChecked.value = true
    } catch (error) {
      startupError.value = `Não foi possível iniciar a Vokia: ${String(error)}`
    }
  }

  async function recheckCli(): Promise<void> {
    cliProblem.value = null
    cliChecked.value = false
    await start()
  }

  function stop(): void {
    unlisten?.()
    unlisten = null
    stopListeningConnection?.()
    stopListeningConnection = null
    if (pollTimer !== null) {
      clearInterval(pollTimer)
      pollTimer = null
    }
  }

  function apply(event: VokiaEvent): void {
    switch (event.type) {
      case 'message':
      case 'error':
        feed.value.push(event)
        break
      case 'agent-state':
        states.value[event.agent] = event.state
        break
      case 'plan-usage':
        break
    }
  }

  function requestMention(agent: Agent): void {
    pendingMention.value = agent
  }

  function takePendingMention(): Agent | null {
    const agent = pendingMention.value
    pendingMention.value = null
    return agent
  }

  function queuedFor(agent: Agent): QueuedRequest[] {
    return queue.value.filter((request) => request.to === agent)
  }

  // Um drain por vez: o backend já roda o fluxo inteiro dentro de send_request, então esperar cada envio mantém a ordem.
  async function drain(): Promise<void> {
    if (draining) {
      return
    }
    draining = true
    try {
      for (let next = queue.value.shift(); next !== undefined; next = queue.value.shift()) {
        try {
          await sendRequest(next.text, next.to)
        } catch (error) {
          feed.value.push({ type: 'error', message: String(error) })
        }
      }
    } finally {
      draining = false
    }
  }

  async function send(text: string, to: Agent | null): Promise<boolean> {
    if (text.trim() === '') {
      return false
    }
    checking.value = true
    try {
      if (!(await refreshConnection())) {
        return false
      }
    } finally {
      checking.value = false
    }
    queue.value.push({
      id: ++nextQueueId,
      text,
      to: to ?? findNamedAgent(text) ?? DEFAULT_RECIPIENT,
    })
    void drain()
    return true
  }

  return {
    cliChecked,
    cliProblem,
    startupError,
    feed,
    timeline,
    states,
    online,
    checking,
    pendingMention,
    queue,
    queuedFor,
    thinkingAgent,
    start,
    recheckCli,
    stop,
    requestMention,
    takePendingMention,
    send,
  }
})
