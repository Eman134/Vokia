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

// Card concluído sem conversa não tem o que mostrar numa thread.
export function showsAsThread(item: { column: BacklogColumn; isTriaging: boolean; messageCount: number }): boolean {
  if (item.isTriaging || ACTIVE_COLUMNS.includes(item.column)) return true
  return item.column === DONE_COLUMN && item.messageCount > 0
}
