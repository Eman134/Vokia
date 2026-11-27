import { computed, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import type { BalloonKind, BalloonSource } from '../balloon/balloonTypes'
import { useProjects } from '../projects/useProjects'
import type { Agent, DecisionKind, FeedUsageLimitFailed, VokiaDecision, VokiaError } from './events'
import { useFeed } from './useFeed'

export type LogKind = 'error' | 'decision' | 'popup'

interface LogEntryBase {
  id: string
  kind: LogKind
  projectId: number | null
  time: number
}

export interface ErrorLogEntry extends LogEntryBase {
  kind: 'error'
  entry: VokiaError | FeedUsageLimitFailed
}

export interface DecisionLogEntry extends LogEntryBase {
  kind: 'decision'
  decisionId: number
  agent: Agent
  decisionKind: DecisionKind
  title: string
  detail: string
  answer?: string
  undone?: boolean
}

export interface PopupLogEntry extends LogEntryBase {
  kind: 'popup'
  balloonKind: BalloonKind
  source: BalloonSource
  title: string
  text?: string
}

export type LogEntry = ErrorLogEntry | DecisionLogEntry | PopupLogEntry

const STORAGE_KEY_PREFIX = 'vokia.logs.read.'
const GLOBAL_PROJECT_KEY = 'global'
const MAX_SESSION_LOGS = 200

function storageKey(projectId: number | null): string {
  return `${STORAGE_KEY_PREFIX}${projectId ?? GLOBAL_PROJECT_KEY}`
}

function readStoredIds(projectId: number | null): string[] {
  try {
    const raw = localStorage.getItem(storageKey(projectId))
    if (raw === null) return []
    const parsed: unknown = JSON.parse(raw)
    return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === 'string') : []
  } catch {
    return []
  }
}

function writeStoredIds(projectId: number | null, ids: readonly string[]): void {
  try {
    localStorage.setItem(storageKey(projectId), JSON.stringify(ids))
  } catch {
    // Sem armazenamento, a leitura vale só nesta sessão.
  }
}

export const useLogs = defineStore('logs', () => {
  const feed = useFeed()
  const projects = useProjects()

  const errorEntries = ref<ErrorLogEntry[]>([])
  const eventEntries = ref<Array<DecisionLogEntry | PopupLogEntry>>([])
  const readByProject = ref<Record<string, Set<string>>>({})
  const seenFeedEntries = new WeakMap<object, number>()
  let errorOccurrence = 0

  const projectId = computed(() => projects.selected?.id ?? null)

  function ensureReadSet(pid: number | null): Set<string> {
    const key = storageKey(pid)
    let set = readByProject.value[key]
    if (set === undefined) {
      set = new Set(readStoredIds(pid))
      readByProject.value = { ...readByProject.value, [key]: set }
    }
    return set
  }

  watch(
    projectId,
    (pid) => {
      ensureReadSet(pid)
    },
    { immediate: true },
  )

  watch(
    () => feed.feed.length,
    () => {
      const next: ErrorLogEntry[] = []
      let errorIndex = 0
      for (const entry of feed.feed) {
        if (entry.type !== 'error' && entry.type !== 'usage-limit-failed') continue
        let time = seenFeedEntries.get(entry)
        if (time === undefined) {
          time = entry.type === 'usage-limit-failed' ? entry.time : Date.now() + errorOccurrence * 0.001
          errorOccurrence += 1
          seenFeedEntries.set(entry, time)
        }
        const id =
          entry.type === 'usage-limit-failed'
            ? `limit:${entry.time}:${entry.agent}`
            : `error:${errorIndex}:${entry.message.slice(0, 80)}`
        errorIndex += 1
        next.push({
          id,
          kind: 'error',
          projectId: projectId.value,
          time,
          entry,
        })
      }
      errorEntries.value = next
    },
    { immediate: true },
  )

  function pushEventEntry(entry: DecisionLogEntry | PopupLogEntry): void {
    if (eventEntries.value.some((candidate) => candidate.id === entry.id)) return
    eventEntries.value = [...eventEntries.value, entry].slice(-MAX_SESSION_LOGS)
  }

  function recordDecision(decision: VokiaDecision): void {
    pushEventEntry({
      id: `decision:${decision.id}`,
      kind: 'decision',
      decisionId: decision.id,
      projectId: decision.projectId ?? projectId.value,
      time: Date.now(),
      agent: decision.agent,
      decisionKind: decision.kind,
      title: decision.title,
      detail: decision.detail,
    })
  }

  function recordDecisionAnswer(decisionId: number, answer: string): void {
    eventEntries.value = eventEntries.value.map((entry) =>
      entry.kind === 'decision' && entry.decisionId === decisionId
        ? { ...entry, answer, undone: false }
        : entry,
    )
  }

  function recordDecisionUndone(decisionId: number): void {
    eventEntries.value = eventEntries.value.map((entry) =>
      entry.kind === 'decision' && entry.decisionId === decisionId
        ? { ...entry, answer: undefined, undone: true }
        : entry,
    )
  }

  function recordPopup(popup: {
    id: string
    kind: BalloonKind
    source: BalloonSource
    title: string
    text?: string
  }): void {
    if (popup.kind === 'error') return
    pushEventEntry({
      id: `popup:${popup.id}`,
      kind: 'popup',
      balloonKind: popup.kind,
      source: popup.source,
      title: popup.title,
      text: popup.text,
      projectId: projectId.value,
      time: Date.now(),
    })
  }

  const items = computed<LogEntry[]>(() => {
    const pid = projectId.value
    const scopedEvents = eventEntries.value.filter((entry) => entry.projectId === null || entry.projectId === pid)
    return [...errorEntries.value, ...scopedEvents].sort((a, b) => b.time - a.time)
  })

  function isRead(id: string): boolean {
    return ensureReadSet(projectId.value).has(id)
  }

  function persistReadSet(pid: number | null, next: Set<string>): void {
    const key = storageKey(pid)
    readByProject.value = { ...readByProject.value, [key]: next }
    writeStoredIds(pid, [...next])
  }

  function markRead(id: string): void {
    const pid = projectId.value
    const current = ensureReadSet(pid)
    if (current.has(id)) return
    const next = new Set(current)
    next.add(id)
    persistReadSet(pid, next)
  }

  function markUnread(id: string): void {
    const pid = projectId.value
    const current = ensureReadSet(pid)
    if (!current.has(id)) return
    const next = new Set(current)
    next.delete(id)
    persistReadSet(pid, next)
  }

  function toggleRead(id: string): void {
    if (isRead(id)) markUnread(id)
    else markRead(id)
  }

  function markAllRead(): void {
    const pid = projectId.value
    const next = new Set(ensureReadSet(pid))
    let changed = false
    for (const item of items.value) {
      if (!next.has(item.id)) {
        next.add(item.id)
        changed = true
      }
    }
    if (changed) persistReadSet(pid, next)
  }

  const unreadCount = computed(() => items.value.filter((item) => !isRead(item.id)).length)
  const unreadErrorCount = computed(
    () => items.value.filter((item) => item.kind === 'error' && !isRead(item.id)).length,
  )

  return {
    items,
    unreadCount,
    unreadErrorCount,
    isRead,
    markRead,
    markUnread,
    toggleRead,
    markAllRead,
    recordDecision,
    recordDecisionAnswer,
    recordDecisionUndone,
    recordPopup,
  }
})
