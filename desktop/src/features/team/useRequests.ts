import { ref, watch } from 'vue'
import { defineStore } from 'pinia'
import type { Agent, VokiaRequestSettled } from './events'
import { DEFAULT_RECIPIENT, findNamedAgent } from './agents'
import {
  enqueueQueuedRequest,
  listQueuedRequests,
  moveQueuedRequest,
  removeQueuedRequest,
  sendRequest,
  updateQueuedRequest,
  type RequestOutcome,
} from './teamApi'
import { useAgentControl } from '../panel/useAgentControl'
import { fetchDbProblem } from '../projects/projectsApi'
import { useProjects } from '../projects/useProjects'
import { useRoster } from './useRoster'
import { projectAgentKey } from './projectAgentKey'
import { ASK_PROJECT_NOTICE, NO_PROJECT_NOTICE, resolveProject } from './projectResolution'
import { useConnection } from './useConnection'
import { useFeed } from './useFeed'

// 'queued' e 'steered' não são desfechos finais: o pedido segue no turno ou aguarda o evento request-settled.
export type RunResult = Exclude<RequestOutcome, 'queued' | 'steered'> | 'cancelled'

export interface QueuedRequest {
  id: number
  text: string
  to: Agent
  projectId: number
  channelId?: number
  itemId?: string
  attachments: string[]
  editing: boolean
  onQueued?: (id: number) => void
  onRejected?: () => void
  onSettled?: (result: RunResult) => void
}

const ATTACHMENTS_OTHER_PROJECT_NOTICE = 'Os anexos são de outro projeto. Selecione o projeto deles para enviar.'

export type QueueMove = 'up' | 'down'

export const useRequests = defineStore('requests', () => {
  const roster = useRoster()
  const pendingMention = ref<Agent | null>(null)
  const queue = ref<QueuedRequest[]>([])
  const control = useAgentControl()
  const projects = useProjects()
  const feed = useFeed()
  const connection = useConnection()
  const loaded = ref(false)
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

  function queuedFor(
    agent: Agent,
    projectId: number | null = projects.selected?.id ?? null,
  ): QueuedRequest[] {
    if (agent === DEFAULT_RECIPIENT) {
      return []
    }
    return queue.value.filter(
      (request) => request.to === agent && (projectId === null || request.projectId === projectId),
    )
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
  async function dispatch(request: QueuedRequest, steer = false): Promise<void> {
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
        request.itemId,
        request.channelId,
        request.attachments,
        steer,
      )
      if (outcome === 'queued') {
        if (request.onSettled === undefined || waiting.has(request.id)) {
          runCallback(() => request.onQueued?.(request.id))
        }
        return
      }
      if (outcome === 'steered') {
        waiting.delete(request.id)
        return
      }
      result = outcome
    } catch (error) {
      feed.feed.push({ type: 'error', message: String(error) })
      runCallback(() => request.onRejected?.())
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
    if (draining || !feed.pausesLoaded || !loaded.value) {
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
        void removeQueuedRequest(head.id).catch((error) => {
          feed.feed.push({ type: 'error', message: `Não foi possível remover o pedido da fila: ${String(error)}` })
        })
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

  async function load(): Promise<void> {
    if (loaded.value) {
      return
    }
    const persisted = await listQueuedRequests()
    const existingIds = new Set(queue.value.map((item) => item.id))
    const restored: QueuedRequest[] = []
    for (const item of persisted) {
      if (existingIds.has(item.id)) {
        continue
      }
      if (item.to === DEFAULT_RECIPIENT) {
        void removeQueuedRequest(item.id).catch(() => {})
        continue
      }
      restored.push({
        id: item.id,
        text: item.text,
        to: item.to,
        projectId: item.projectId,
        channelId: item.channelId ?? undefined,
        itemId: item.itemId ?? undefined,
        attachments: item.attachments,
        editing: false,
      })
    }
    queue.value = [...restored, ...queue.value]
    loaded.value = true
    void drain()
  }

  watch(
    [() => control.paused, () => feed.pausesLoaded, loaded],
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
    if (target === undefined || (text.trim() === '' && target.attachments.length === 0)) {
      return false
    }
    target.text = text
    target.editing = false
    void updateQueuedRequest(id, text).catch((error) => {
      feed.feed.push({ type: 'error', message: `Não foi possível salvar a edição da fila: ${String(error)}` })
    })
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
      void moveQueuedRequest(id, move).catch((error) => {
        feed.feed.push({ type: 'error', message: `Não foi possível reordenar a fila: ${String(error)}` })
      })
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
      void removeQueuedRequest(id).catch((error) => {
        feed.feed.push({ type: 'error', message: `Não foi possível remover o pedido da fila: ${String(error)}` })
      })
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

  async function targetProjectId(text: string, recipient: Agent): Promise<number | null> {
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
    // Ember delega entre projetos diretamente pela ActionBar sem ser bloqueado por trabalho em andamento em outro projeto.
    if (recipient === DEFAULT_RECIPIENT) {
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

  // IDs imediatos são inteiros positivos altos para nunca colidir com os IDs do SQLite nem violar u64 no backend.
  let nextImmediateId = Date.now() * 1000

  async function send(
    text: string,
    to: Agent | null,
    options?: {
      projectId?: number
      channelId?: number
      itemId?: string
      attachments?: string[]
      attachmentsProjectId?: number | null
      force?: 'now' | 'queue'
      onQueued?: (id: number) => void
      onRejected?: () => void
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
    const attachments = options?.attachments ?? []
    if (text.trim() === '' && attachments.length === 0) {
      return false
    }
    if (!(await connection.confirmOnline())) {
      return false
    }
    const screenProjectId = projects.selected?.id
    const recipient = to ?? findNamedAgent(text) ?? DEFAULT_RECIPIENT
    const projectId = options?.projectId ?? (await targetProjectId(text, recipient))
    if (projectId === null) {
      return false
    }
    if (attachments.length > 0 && projectId !== options?.attachmentsProjectId) {
      feed.notify(ATTACHMENTS_OTHER_PROJECT_NOTICE)
      return false
    }
    // Um id de canal só vale no projeto da tela; texto que aponta outro projeto vai sem canal.
    const channelId = projectId === screenProjectId ? options?.channelId : undefined
    const itemId = projectId === screenProjectId ? options?.itemId : undefined

    // Ember nunca entra em fila; e mensagens com force === 'now' furam a fila e entram imediatamente (steer).
    if (recipient === DEFAULT_RECIPIENT || options?.force === 'now') {
      const immediateId = nextImmediateId++
      const directRequest: QueuedRequest = {
        id: immediateId,
        text,
        to: recipient,
        projectId,
        channelId,
        itemId,
        attachments,
        editing: false,
        onQueued: options?.onQueued,
        onRejected: options?.onRejected,
        onSettled: options?.onSettled,
      }
      options?.onEnqueued?.({
        id: immediateId,
        to: recipient,
        ahead: 0,
        dispatched: true,
        externalBusy: false,
      })
      void dispatch(directRequest, options?.force === 'now')
      return true
    }

    const paused = control.isPaused(recipient, projectId)
    const waitingBefore = queue.value.filter(isDispatchable).length
    const busyState = feed.activity[projectAgentKey(projectId, recipient)]
    const runningHere = inFlight !== null && inFlight.to === recipient && inFlight.projectId === projectId
    const externalBusy = (busyState === 'sending' || busyState === 'thinking') && !runningHere
    const ahead = paused ? null : (inFlight === null ? 0 : 1) + waitingBefore + (externalBusy ? 1 : 0)
    const record = await enqueueQueuedRequest({
      projectId,
      to: recipient,
      text,
      channelId,
      itemId,
      attachments,
    })
    const id = record.id
    queue.value.push({
      id,
      text,
      to: recipient,
      projectId,
      channelId,
      itemId,
      attachments,
      editing: false,
      onQueued: options?.onQueued,
      onRejected: options?.onRejected,
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
    load,
    send,
    startEdit,
    saveEdit,
    cancelEdit,
    moveQueued,
    removeQueued,
  }
})
