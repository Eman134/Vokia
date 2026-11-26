import type { BacklogColumn } from '../backlog/backlogTypes'

export type ThreadStatus = 'working' | 'needs-you' | 'idle' | 'done'

const DONE_COLUMN: BacklogColumn = 'Done'
const ACTIVE_COLUMNS: readonly BacklogColumn[] = ['In progress', 'In review']

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

export type ThreadCategoryKey = 'needs-you' | 'triage' | 'in-progress' | 'in-review' | 'recent' | 'done'

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
  const triage: T[] = []
  const inProgress: T[] = []
  const inReview: T[] = []
  const done: T[] = []

  for (const thread of threads) {
    if (thread.status === 'done') {
      done.push(thread)
    } else if (thread.status === 'needs-you') {
      needsYou.push(thread)
    } else if (thread.isTriaging || thread.column === 'Backlog' || thread.column === 'Ready') {
      triage.push(thread)
    } else if (thread.column === 'In review') {
      inReview.push(thread)
    } else {
      inProgress.push(thread)
    }
  }

  const recent = [...threads]
    .filter((thread) => thread.lastActivity > 0)
    .sort((a, b) => b.lastActivity - a.lastActivity)
    .slice(0, RECENT_LIMIT)

  const categories: ThreadCategory<T>[] = [
    { key: 'needs-you', label: 'Precisa de você', items: needsYou, defaultOpen: true },
    { key: 'triage', label: 'Em triagem', items: triage, defaultOpen: true },
    { key: 'in-progress', label: 'Em andamento', items: inProgress, defaultOpen: true },
    { key: 'in-review', label: 'Em revisão', items: inReview, defaultOpen: true },
    { key: 'recent', label: 'Recentes', items: recent, defaultOpen: false },
    { key: 'done', label: 'Concluídas', items: done, defaultOpen: false },
  ]
  return categories.filter((category) => category.items.length > 0)
}

// Card concluído sem conversa não tem o que mostrar numa thread.
export function showsAsThread(item: { column: BacklogColumn; isTriaging: boolean; messageCount: number }): boolean {
  if (item.isTriaging || ACTIVE_COLUMNS.includes(item.column)) return true
  return item.column === DONE_COLUMN && item.messageCount > 0
}
