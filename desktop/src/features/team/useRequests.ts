import { ref, watch } from 'vue'
import { defineStore } from 'pinia'
import type { Agent, VokiaRequestSettled } from './events'
import { DEFAULT_RECIPIENT, findNamedAgent } from './agents'
import { sendRequest, type RequestOutcome } from './teamApi'
import { useAgentControl } from '../panel/useAgentControl'
import { fetchDbProblem } from '../projects/projectsApi'
import { useProjects } from '../projects/useProjects'
import { useRoster } from './useRoster'
import { projectAgentKey } from './projectAgentKey'
import { ASK_PROJECT_NOTICE, NO_PROJECT_NOTICE, resolveProject } from './projectResolution'
import { useConnection } from './useConnection'
import { useFeed } from './useFeed'

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

export const useRequests = defineStore('requests', () => {
  const roster = useRoster()
  const pendingMention = ref<Agent | null>(null)
  const queue = ref<QueuedRequest[]>([])
  const control = useAgentControl()
  const projects = useProjects()
  const feed = useFeed()
  const connection = useConnection()
  let nextQueueId = 0
  let draining = false
  let inFlight: QueuedRequest | null = null
  // Callbacks de pedidos com onSettled, por id, até o término (resposta do comando ou evento request-settled).
  const waiting = new Map<number, (result: RunResult) => void>()

  function applySettled(event: VokiaRequestSettled): void {
    const settle = waiting.get(event.runId)
    waiting.delete(event.runId)
    if (settle !== undefined) runCallback(() => settle(event.outcome))
  }

  feed.onRequestSettled(applySettled)

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
      feed.feed.push({ type: 'error', message: `Não foi possível atualizar o pedido: ${String(error)}` })
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
      feed.feed.push({ type: 'error', message: String(error) })
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
    if (draining || !feed.pausesLoaded) {
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
    [() => control.paused, () => feed.pausesLoaded],
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

  function hasPendingWork(projectId: number): boolean {
    return (
      queue.value.some((request) => request.projectId === projectId) ||
      roster.agents.some((agent) => feed.projectsOf(agent.id).includes(projectId))
    )
  }

  // Sem projeto porque o banco não abriu, o motivo do banco é o que o usuário precisa ver.
  async function notifyMissingProject(): Promise<void> {
    try {
      const problem = await fetchDbProblem()
      if (problem !== null) {
        feed.feed.push({ type: 'error', message: problem.reason })
        return
      }
    } catch (error) {
      feed.feed.push({ type: 'error', message: String(error) })
      return
    }
    feed.notify(NO_PROJECT_NOTICE)
  }

  async function targetProjectId(text: string): Promise<number | null> {
    const selected = projects.selected
    const resolution = resolveProject(text, projects.projects, selected?.id ?? null)
    if (resolution.kind === 'ask') {
      feed.notify(ASK_PROJECT_NOTICE)
      return null
    }
    if (resolution.kind === 'no-project') {
      await notifyMissingProject()
      return null
    }
    const { project } = resolution
    if (project.id === selected?.id) {
      return project.id
    }
    if (selected !== null && hasPendingWork(selected.id)) {
      feed.notify(`Há agentes trabalhando em #${selected.name}. Espere terminar para enviar um pedido a #${project.name}.`)
      return null
    }
    try {
      await projects.select(project.id)
    } catch (error) {
      feed.feed.push({ type: 'error', message: String(error) })
      return null
    }
    await feed.restoreHistory()
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
    if (!(await connection.confirmOnline())) {
      return false
    }
    const projectId = options?.projectId ?? (await targetProjectId(text))
    if (projectId === null) {
      return false
    }
    const recipient = to ?? findNamedAgent(text) ?? DEFAULT_RECIPIENT
    const id = ++nextQueueId
    const paused = control.isPaused(recipient, projectId)
    const waitingBefore = queue.value.filter(isDispatchable).length
    const busyState = feed.activity[projectAgentKey(projectId, recipient)]
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
    pendingMention,
    queue,
    queuedFor,
    requestMention,
    takePendingMention,
    send,
    startEdit,
    saveEdit,
    cancelEdit,
    moveQueued,
    removeQueued,
  }
})
