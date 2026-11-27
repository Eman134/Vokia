import type { BacklogColumn } from '../backlog/backlogTypes'

export type ThreadStatus = 'working' | 'needs-you' | 'idle' | 'done'

const DONE_COLUMN: BacklogColumn = 'Done'
const THREAD_COLUMNS: readonly BacklogColumn[] = ['Ready', 'In progress', 'In review']
export const DONE_THREAD_RETENTION_MS = 24 * 60 * 60 * 1000

interface StatusInput {
  column: BacklogColumn
  isTriaging: boolean
  hasOpenQuestion: boolean
  hasPendingPermission: boolean
  hasWorkingAgent: boolean
}

// O que pede ação do usuário vale mais que o agente trabalhando: é o que a lista precisa destacar.
export function threadStatus(input: StatusInput): ThreadStatus {
  if (input.column === DONE_COLUMN) return 'done'
  if (input.hasOpenQuestion || input.hasPendingPermission) return 'needs-you'
  return input.isTriaging || input.hasWorkingAgent ? 'working' : 'idle'
}

export function splitThreads<T extends { status: ThreadStatus }>(threads: readonly T[]): { active: T[]; done: T[] } {
  return {
    active: threads.filter((thread) => thread.status !== 'done'),
    done: threads.filter((thread) => thread.status === 'done'),
  }
}

export type ThreadCategoryKey = 'recent' | 'needs-you' | 'in-progress' | 'in-review' | 'ready' | 'done'

export interface ThreadCategory<T> {
  key: ThreadCategoryKey
  label: string
  items: T[]
  defaultOpen: boolean
}

const RECENT_LIMIT = 3

export function categorizeThreads<
  T extends { status: ThreadStatus; column: BacklogColumn; isTriaging?: boolean; lastActivity: number },
>(threads: readonly T[]): ThreadCategory<T>[] {
  const needsYou: T[] = []
  const inProgress: T[] = []
  const inReview: T[] = []
  const ready: T[] = []
  const done: T[] = []

  for (const thread of threads) {
    if (thread.status === 'done') {
      done.push(thread)
    } else if (thread.status === 'needs-you') {
      needsYou.push(thread)
    } else if (thread.column === 'In review') {
      inReview.push(thread)
    } else if (thread.column === 'Ready') {
      ready.push(thread)
    } else {
      inProgress.push(thread)
    }
  }

  const recent = [...threads]
    .filter((thread) => thread.status !== 'done' && thread.lastActivity > 0)
    .sort((a, b) => b.lastActivity - a.lastActivity)
    .slice(0, RECENT_LIMIT)

  const categories: ThreadCategory<T>[] = [
    { key: 'recent', label: 'Recentes', items: recent, defaultOpen: true },
    { key: 'needs-you', label: 'Precisa de você', items: needsYou, defaultOpen: true },
    { key: 'in-progress', label: 'Em andamento', items: inProgress, defaultOpen: true },
    { key: 'in-review', label: 'Em revisão', items: inReview, defaultOpen: true },
    { key: 'ready', label: 'Pronto', items: ready, defaultOpen: false },
    { key: 'done', label: 'Concluídas', items: done, defaultOpen: false },
  ]
  return categories.filter((category) => category.items.length > 0)
}

// Triagem fica no canal #triagem (Backlog); cards em Ready/In progress/In review aparecem em Threads e Done fica visível até arquivar.
export function showsAsThread(
  item: {
    column: BacklogColumn
    isTriaging: boolean
    messageCount: number
    lastActivity?: number
    hasSubthreadMessages?: boolean
  },
  now = Date.now(),
): boolean {
  if (item.column === 'Backlog') return false
  if (THREAD_COLUMNS.includes(item.column)) return true
  if (item.column === DONE_COLUMN && (item.messageCount > 0 || item.hasSubthreadMessages)) {
    const last = item.lastActivity ?? 0
    return last === 0 || now - last <= DONE_THREAD_RETENTION_MS
  }
  return false
}
