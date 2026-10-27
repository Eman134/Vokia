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

const CONNECTIVITY_POLL_INTERVAL_MS = 15000
export const DEFAULT_RECIPIENT: Agent = 'iara'
const MENTION_PATTERN = /^@(\S+)(?:\s+|$)/

export interface AgentProfile {
  id: Agent
  name: string
  role: string
}

export type FeedEntry = VokiaMessage | VokiaError

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

export interface ParsedMention {
  agent: Agent
  text: string
}

export function parseMention(text: string): ParsedMention | null {
  const match = MENTION_PATTERN.exec(text)
  if (match === null) {
    return null
  }
  const [mention, name] = match
  const profile = AGENTS.find((agent) => agent.name.toLowerCase() === name.toLowerCase())
  if (profile === undefined) {
    return null
  }
  return { agent: profile.id, text: text.slice(mention.length) }
}

export const useTeam = defineStore('team', () => {
  const cliChecked = ref(false)
  const cliProblem = ref<CliProblem | null>(null)
  const startupError = ref<string | null>(null)
  const feed = ref<FeedEntry[]>([])
  const states = ref<Record<Agent, AgentState>>({ iara: 'idle', davi: 'idle' })
  const busy = ref(false)
  const online = ref(true)
  const checking = ref(false)
  const recipient = ref<Agent | null>(null)
  let offlineSignals = 0
  let unlisten: UnlistenFn | null = null
  let stopListeningConnection: (() => void) | null = null
  let pollTimer: ReturnType<typeof setInterval> | null = null
  let pendingCheck: Promise<boolean> | null = null

  const thinkingAgent = computed<AgentProfile | undefined>(() =>
    AGENTS.find((agent) => states.value[agent.id] === 'thinking'),
  )
  const statusText = computed<string>(() =>
    thinkingAgent.value ? `${thinkingAgent.value.name} pensando` : 'Tudo em dia',
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

  async function start(): Promise<void> {
    try {
      cliProblem.value = await fetchCliProblem()
      if (cliProblem.value === null) {
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
    }
  }

  function chooseRecipient(agent: Agent): void {
    recipient.value = agent
  }

  function clearRecipient(): void {
    recipient.value = null
  }

  async function send(text: string): Promise<boolean> {
    const mention = parseMention(text)
    const body = mention?.text ?? text
    if (body.trim() === '') {
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
    if (mention !== null) {
      recipient.value = mention.agent
    }
    busy.value = true
    try {
      await sendRequest(body, recipient.value ?? DEFAULT_RECIPIENT)
    } catch (error) {
      feed.value.push({ type: 'error', message: String(error) })
    } finally {
      busy.value = false
    }
    return true
  }

  return {
    cliChecked,
    cliProblem,
    startupError,
    feed,
    timeline,
    states,
    busy,
    online,
    checking,
    recipient,
    thinkingAgent,
    statusText,
    start,
    stop,
    chooseRecipient,
    clearRecipient,
    send,
  }
})
