import type { ConsoleEntry, ConsoleKind } from '../team/events'

export type ConsoleFilter = 'all' | 'tools' | 'errors'

export const CONSOLE_FILTER_LABEL = 'Filtrar saída'
export const CONSOLE_FILTERS = [
  { label: 'Tudo', value: 'all' },
  { label: 'Ferramentas', value: 'tools' },
  { label: 'Erros', value: 'errors' },
] as const

const FILTER_KINDS: Record<ConsoleFilter, readonly ConsoleKind[] | null> = {
  all: null,
  tools: ['tool', 'output', 'permission'],
  errors: ['error', 'failed'],
}

export function visibleEntries(entries: readonly ConsoleEntry[], filter: ConsoleFilter): readonly ConsoleEntry[] {
  const kinds = FILTER_KINDS[filter]
  return kinds === null ? entries : entries.filter((entry) => kinds.includes(entry.kind))
}
