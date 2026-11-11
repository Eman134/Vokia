import { computed, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import {
  type Agent,
  type AgentState,
  type CliProblem,
  type FeedNotice,
  type FeedPermissionResult,
  type VokiaAgentState,
  type VokiaError,
  type VokiaEvent,
  type VokiaMessage,
  type VokiaPermission,
} from './events'
import {
  answerPermission,
  checkConnection,
  fetchCliProblem,
  listenConnectionChanges,
  listenTeamEvents,
  sendRequest,
  type AgentProfile,
  type RequestOutcome,
  type UnlistenFn,
} from './teamApi'
import { useDecisions } from '../decisions/useDecisions'
import { useAgentControl } from '../panel/useAgentControl'
import { clearConversation, fetchDbProblem, loadHistory } from '../projects/projectsApi'
import { useProjects } from '../projects/useProjects'
import { pauseNotice } from '../panel/nowCopy'
import { normalizeName, useRoster } from './useRoster'
import { projectAgentKey } from './projectAgentKey'
import { ASK_PROJECT_NOTICE, NO_PROJECT_NOTICE, resolveProject } from './projectResolution'

const CONNECTIVITY_POLL_INTERVAL_MS = 15000
export const DEFAULT_RECIPIENT: Agent = 'iara'

export function agentName(id: Agent): string {
  return useRoster().nameOf(id)
}

export type FeedEntry = VokiaMessage | VokiaError | FeedNotice | FeedPermissionResult

// 'queued' não é desfecho: o término do pedido enfileirado chega depois pelo evento request-settled.
export type RunResult = Exclude<RequestOutcome, 'queued'> | 'cancelled'

export interface QueuedRequest {
  id: number
  text: string
  to: Agent
  projectId: number
  editing: boolean
  onQueued?: (id: number) => void
  onSettled?: (result: RunResult) => void
}

export type QueueMove = 'up' | 'down'

export type TimelineItem =
  | { type: 'entry'; start: number; entry: FeedEntry }
  | { type: 'group'; start: number; messages: VokiaMessage[] }

export function isBetweenAgents(message: VokiaMessage): boolean {
  return message.from !== 'user' && message.to !== 'user'
}

export function findNamedAgent(text: string): Agent | null {
  let found: { agent: Agent; index: number } | null = null
  const normalizedText = normalizeName(text)
  for (const profile of useRoster().agents) {
    const index = normalizedText.search(new RegExp(`(?<!\\p{L})${normalizeName(profile.name)}(?!\\p{L})`, 'u'))
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
  const roster = useRoster()
  const states = ref<Record<Agent, AgentState>>({})
  const online = ref(true)
  const checking = ref(false)
  const pendingMention = ref<Agent | null>(null)
  const queue = ref<QueuedRequest[]>([])
  const permissions = ref<VokiaPermission[]>([])
  const control = useAgentControl()
  const projects = useProjects()
  const pausesLoaded = ref(false)
  const activity = ref<Record<string, AgentState>>({})
  const lastProject = ref<Record<Agent, number | null>>({})
  let offlineSignals = 0
  let unlisten: UnlistenFn | null = null
  let stopListeningConnection: (() => void) | null = null
  let pollTimer: ReturnType<typeof setInterval> | null = null
  let pendingCheck: Promise<boolean> | null = null
  let nextQueueId = 0
  let draining = false
  let inFlight: QueuedRequest | null = null
  // Callbacks de pedidos com onSettled, por id, até o término (resposta do comando ou evento request-settled).
  const waiting = new Map<number, (result: RunResult) => void>()

  function stateOf(agent: Agent): AgentState {
    return states.value[agent] ?? 'idle'
  }

  const thinkingAgent = computed<AgentProfile | undefined>(() =>
    roster.agents.find((agent) => stateOf(agent.id) === 'thinking'),
  )
  const activeAgents = computed<AgentProfile[]>(() => roster.agents.filter((agent) => stateOf(agent.id) !== 'idle'))
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
      await projects.load()
      await restorePauses()
      if (projects.selected !== null) {
        feed.value = await loadHistory(projects.selected.id)
      }
    } catch (error) {
      feed.value.push({
        type: 'error',
        message: `Não foi possível carregar o histórico: ${String(error)}`,
      })
    }
  }

  async function clearFeed(): Promise<void> {
    const project = projects.selected
    if (project === null) {
      return
    }
    await clearConversation(project.id)
    feed.value = []
  }

  async function restorePauses(): Promise<void> {
    try {
      await control.load()
      pausesLoaded.value = true
      void drain()
    } catch (error) {
      feed.value.push({
        type: 'error',
        message: `Não foi possível carregar as pausas dos agentes: ${String(error)}`,
      })
    }
  }

  async function start(): Promise<void> {
    try {
      await roster.load()
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
    useDecisions().stop()
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
        recordActivity(event)
        break
      case 'permission':
        permissions.value.push(event)
        break
      case 'permission-closed': {
        const closed = permissions.value.find((permission) => permission.id === event.id)
        if (closed) {
          feed.value.push({
            type: 'permission-result',
            agent: closed.agent,
            detail: closed.detail,
            outcome: event.outcome,
            time: Date.now(),
          })
        }
        permissions.value = permissions.value.filter((permission) => permission.id !== event.id)
        break
      }
      case 'decision':
        useDecisions().apply(event)
        break
      case 'request-settled': {
        const settle = waiting.get(event.runId)
        waiting.delete(event.runId)
        if (settle !== undefined) runCallback(() => settle(event.outcome))
        break
      }
      case 'plan-usage':
        break
    }
  }

  function recordActivity({ agent, state, projectId }: VokiaAgentState): void {
    if (projectId === null) {
      return
    }
    lastProject.value[agent] = projectId
    activity.value[projectAgentKey(projectId, agent)] = state
  }

  function projectsOf(agent: Agent): number[] {
    return Object.entries(activity.value)
      .filter(([key, state]) => state !== 'idle' && key.endsWith(`:${agent}`))
      .map(([key]) => Number(key.split(':')[0]))
  }

  async function togglePause(agent: Agent): Promise<void> {
    try {
      const pausing = !control.isPaused(agent)
      await (pausing ? control.pause(agent) : control.resume(agent))
      feed.value.push({ type: 'notice', text: pauseNotice(agentName(agent), pausing), time: Date.now() })
    } catch (error) {
      feed.value.push({ type: 'error', message: String(error) })
    }
  }

  async function decidePermission(id: string, allow: boolean): Promise<void> {
    try {
      await answerPermission(id, allow)
    } catch (error) {
      feed.value.push({ type: 'error', message: String(error) })
      permissions.value = permissions.value.filter((permission) => permission.id !== id)
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

  // Um callback que lança não pode parar a fila nem o laço de eventos: o erro vai ao feed e o fluxo segue.
  function runCallback(callback: () => void): void {
    try {
      callback()
    } catch (error) {
      feed.value.push({ type: 'error', message: `Não foi possível atualizar o pedido: ${String(error)}` })
    }
  }

  // O callback é registrado antes do envio: o evento request-settled pode chegar antes da resposta do comando.
  async function dispatch(request: QueuedRequest): Promise<void> {
    if (request.onSettled !== undefined) {
      waiting.set(request.id, request.onSettled)
    }
    let result: RunResult = 'failed'
    try {
      const outcome = await sendRequest(
        request.text,
        request.to,
        request.projectId,
        request.onSettled !== undefined ? request.id : undefined,
      )
      if (outcome === 'queued') {
        if (request.onSettled === undefined || waiting.has(request.id)) {
          runCallback(() => request.onQueued?.(request.id))
        }
        return
      }
      result = outcome
    } catch (error) {
      feed.value.push({ type: 'error', message: String(error) })
    }
    const settle = waiting.get(request.id)
    waiting.delete(request.id)
    if (settle !== undefined) runCallback(() => settle(result))
  }

  function isDispatchable(request: QueuedRequest): boolean {
    return !request.editing && !control.isPaused(request.to, request.projectId)
  }

  // Um drain por vez: o backend já roda o fluxo inteiro dentro de send_request, então esperar cada envio mantém a ordem.
  async function drain(): Promise<void> {
    if (draining || !pausesLoaded.value) {
      return
    }
    draining = true
    try {
      for (;;) {
        const index = queue.value.findIndex(isDispatchable)
        const head = queue.value[index]
        if (head === undefined) {
          break
        }
        queue.value.splice(index, 1)
        inFlight = head
        try {
          await dispatch(head)
        } finally {
          inFlight = null
        }
      }
    } finally {
      draining = false
    }
  }

  watch(
    () => control.paused,
    () => void drain(),
  )

  function startEdit(id: number): boolean {
    const target = queue.value.find((request) => request.id === id)
    if (target === undefined) {
      return false
    }
    for (const request of queue.value) {
      request.editing = request.id === id
    }
    void drain()
    return true
  }

  function saveEdit(id: number, text: string): boolean {
    const target = queue.value.find((request) => request.id === id)
    if (target === undefined || text.trim() === '') {
      return false
    }
    target.text = text
    target.editing = false
    void drain()
    return true
  }

  function cancelEdit(id: number): void {
    const target = queue.value.find((request) => request.id === id)
    if (target !== undefined) {
      target.editing = false
    }
    void drain()
  }

  function moveQueued(id: number, move: QueueMove): void {
    const index = queue.value.findIndex((request) => request.id === id)
    const neighbor = move === 'up' ? index - 1 : index + 1
    if (index !== -1 && neighbor >= 0 && neighbor < queue.value.length) {
      const moved = queue.value[index]!
      queue.value[index] = queue.value[neighbor]!
      queue.value[neighbor] = moved
    }
    void drain()
  }

  function removeQueued(id: number): void {
    const index = queue.value.findIndex((request) => request.id === id)
    if (index !== -1) {
      const cancelled = queue.value[index]
      if (cancelled?.onSettled !== undefined) {
        const settle = cancelled.onSettled
        runCallback(() => settle('cancelled'))
      }
      queue.value.splice(index, 1)
    }
    void drain()
  }

  function notify(text: string): void {
    feed.value.push({ type: 'notice', text, time: Date.now() })
  }

  function hasPendingWork(projectId: number): boolean {
    return (
      queue.value.some((request) => request.projectId === projectId) ||
      roster.agents.some((agent) => projectsOf(agent.id).includes(projectId))
    )
  }

  async function targetProjectId(text: string): Promise<number | null> {
    const selected = projects.selected
    const resolution = resolveProject(text, projects.projects, selected?.id ?? null)
    if (resolution.kind === 'ask') {
      notify(ASK_PROJECT_NOTICE)
      return null
    }
    if (resolution.kind === 'no-project') {
      notify(NO_PROJECT_NOTICE)
      return null
    }
    const { project } = resolution
    if (project.id === selected?.id) {
      return project.id
    }
    if (selected !== null && hasPendingWork(selected.id)) {
      notify(`Há agentes trabalhando em #${selected.name}. Espere terminar para enviar um pedido a #${project.name}.`)
      return null
    }
    try {
      await projects.select(project.id)
    } catch (error) {
      feed.value.push({ type: 'error', message: String(error) })
      return null
    }
    await restoreHistory()
    return project.id
  }

  async function send(
    text: string,
    to: Agent | null,
    options?: {
      projectId?: number
      onQueued?: (id: number) => void
      onSettled?: (result: RunResult) => void
      onEnqueued?: (info: {
        id: number
        to: Agent
        ahead: number | null
        dispatched: boolean
        externalBusy: boolean
      }) => void
    },
  ): Promise<boolean> {
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
    const projectId = options?.projectId ?? (await targetProjectId(text))
    if (projectId === null) {
      return false
    }
    const recipient = to ?? findNamedAgent(text) ?? DEFAULT_RECIPIENT
    const id = ++nextQueueId
    const paused = control.isPaused(recipient, projectId)
    const waitingBefore = queue.value.filter(isDispatchable).length
    const busyState = activity.value[projectAgentKey(projectId, recipient)]
    const runningHere = inFlight !== null && inFlight.to === recipient && inFlight.projectId === projectId
    const externalBusy = (busyState === 'sending' || busyState === 'thinking') && !runningHere
    const ahead = paused ? null : (inFlight === null ? 0 : 1) + waitingBefore + (externalBusy ? 1 : 0)
    queue.value.push({
      id,
      text,
      to: recipient,
      projectId,
      editing: false,
      onQueued: options?.onQueued,
      onSettled: options?.onSettled,
    })
    void drain()
    options?.onEnqueued?.({
      id,
      to: recipient,
      ahead,
      dispatched: !queue.value.some((request) => request.id === id),
      externalBusy,
    })
    return true
  }

  return {
    cliChecked,
    cliProblem,
    startupError,
    feed,
    timeline,
    states,
    stateOf,
    activity,
    lastProject,
    projectsOf,
    togglePause,
    online,
    checking,
    pendingMention,
    queue,
    permissions,
    decidePermission,
    queuedFor,
    startEdit,
    saveEdit,
    cancelEdit,
    moveQueued,
    removeQueued,
    thinkingAgent,
    activeAgents,
    start,
    recheckCli,
    stop,
    requestMention,
    takePendingMention,
    send,
    restoreHistory,
    clearFeed,
  }
})
