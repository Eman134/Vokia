import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { useBalloons } from '../balloon/useBalloons'
import { useProjects } from '../projects/useProjects'
import type { Agent, VokiaDecision } from '../team/events'
import { answerDecision, dismissDecision } from '../team/teamApi'
import { useFeed } from '../team/useFeed'
import { useLogs } from '../team/useLogs'
import { UNDO_WINDOW_MS, type CardKind } from './decisionCopy'

export interface PanelCard {
  key: string
  kind: CardKind
  agent: Agent
  title: string
  detail: string
  options: string[]
  decisionId: number | null
  permissionId: string | null
  itemId: string | null
}

export interface ResolvedEntry {
  key: string
  message: string
  decisionId: number | null
  sent: boolean
}

interface Awaiting {
  decision: VokiaDecision
  answer: string | null
  timer: ReturnType<typeof setTimeout>
}

export interface AnswerLogEntry {
  decisionId: number
  text: string
  kind: 'answered' | 'undone'
}

const ANSWER_LOG_LIMIT = 200
const DECISION_PENDING_TITLE = 'Decisão pendente'

export const useDecisions = defineStore('decisions', () => {
  const feed = useFeed()
  const balloons = useBalloons()
  const projects = useProjects()
  const logs = useLogs()
  const pending = ref<VokiaDecision[]>([])
  const resolved = ref<ResolvedEntry[]>([])
  const answerLog = ref<AnswerLogEntry[]>([])
  // Contador total de entradas: o observador do log usa a diferença dele, pois o array é aparado.
  const answerLogTotal = ref(0)
  const open = ref(false)
  const awaiting = new Map<number, Awaiting>()

  function belongsToSelectedProject(projectId: number | null): boolean {
    return projectId === null || projectId === (projects.selected?.id ?? null)
  }

  const cards = computed<PanelCard[]>(() => [
    ...feed.projectPermissions.map((permission) => ({
      key: `permissao-${permission.id}`,
      kind: 'permissao' as const,
      agent: permission.agent,
      title: permission.tool,
      detail: permission.detail,
      options: [],
      decisionId: null,
      permissionId: permission.id,
      itemId: null,
    })),
    ...pending.value
      .filter((decision) => belongsToSelectedProject(decision.projectId))
      .map((decision) => ({
        key: `decisao-${decision.id}`,
        kind: decision.kind,
        agent: decision.agent,
        title: decision.title,
        detail: decision.detail,
        options: decision.options,
        decisionId: decision.id,
        permissionId: null,
        itemId: decision.itemId ?? null,
      })),
  ])
  const pendingCount = computed(() => cards.value.length)

  function apply(event: VokiaDecision): void {
    if (!pending.value.some((decision) => decision.id === event.id)) {
      pending.value.push(event)
      if (belongsToSelectedProject(event.projectId)) {
        logs.recordDecision(event)
      }
      // Com o painel aberto a decisão já está à vista.
      if (!open.value && belongsToSelectedProject(event.projectId)) {
        balloons.alert(DECISION_PENDING_TITLE, event.title, 'conversa', { log: false })
      }
    }
  }

  function logAnswer(entry: AnswerLogEntry): void {
    answerLog.value = [...answerLog.value, entry].slice(-ANSWER_LOG_LIMIT)
    answerLogTotal.value += 1
  }

  function answer(id: number, text: string, message: string): void {
    const decision = pending.value.find((candidate) => candidate.id === id)
    if (decision === undefined) {
      return
    }
    pending.value = pending.value.filter((candidate) => candidate.id !== id)
    resolved.value.push({ key: `decisao-${id}`, message, decisionId: id, sent: false })
    logAnswer({ decisionId: id, text, kind: 'answered' })
    logs.recordDecisionAnswer(id, text)
    awaiting.set(id, { decision, answer: text, timer: setTimeout(() => void send(id), UNDO_WINDOW_MS) })
  }

  function dismiss(id: number, message = 'Decisão ignorada'): void {
    const decision = pending.value.find((candidate) => candidate.id === id)
    if (decision === undefined) {
      return
    }
    pending.value = pending.value.filter((candidate) => candidate.id !== id)
    resolved.value.push({ key: `decisao-${id}`, message, decisionId: id, sent: false })
    logAnswer({ decisionId: id, text: '', kind: 'answered' })
    awaiting.set(id, { decision, answer: null, timer: setTimeout(() => void send(id), UNDO_WINDOW_MS) })
  }

  async function send(id: number): Promise<void> {
    const entry = awaiting.get(id)
    if (entry === undefined) {
      return
    }
    awaiting.delete(id)
    const record = resolved.value.find((candidate) => candidate.decisionId === id)
    if (record !== undefined) {
      record.sent = true
    }
    try {
      if (entry.answer === null) {
        await dismissDecision(id)
      } else {
        await answerDecision(id, entry.answer)
      }
    } catch (error) {
      feed.feed.push({ type: 'error', message: String(error) })
    }
  }

  function undo(id: number): void {
    const entry = awaiting.get(id)
    if (entry === undefined) {
      return
    }
    clearTimeout(entry.timer)
    awaiting.delete(id)
    resolved.value = resolved.value.filter((candidate) => candidate.decisionId !== id)
    pending.value = [...pending.value, entry.decision].sort((a, b) => a.id - b.id)
    logAnswer({ decisionId: id, text: '', kind: 'undone' })
    logs.recordDecisionUndone(id)
  }

  function answerPermission(id: string, allow: boolean, message: string, always = false): void {
    void feed.decidePermission(id, allow, always)
    resolved.value.push({ key: `permissao-${id}`, message, decisionId: null, sent: true })
  }

  function setOpen(next: boolean): void {
    open.value = next
    balloons.setPanelOpen(next)
    if (!next) {
      resolved.value = resolved.value.filter((entry) => !entry.sent)
    }
  }

  function toggle(): void {
    setOpen(!open.value)
  }

  function stop(): void {
    for (const entry of awaiting.values()) {
      clearTimeout(entry.timer)
    }
    awaiting.clear()
  }

  return { pending, resolved, answerLog, answerLogTotal, cards, pendingCount, open, apply, answer, dismiss, undo, answerPermission, setOpen, toggle, stop }
})
