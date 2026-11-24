import type { BacklogColumn, BacklogState } from '../backlog/backlogTypes'
import { plainSummary } from './artifactsCopy'

export type ArtifactGroupId = 'precisa' | 'execucao' | 'prontos' | 'concluidos' | 'documentos'
export type StateFilter = 'todos' | 'proposta' | 'pronto' | 'execucao' | 'concluido'

export interface BrowsableArtifact {
  key: string
  id: string
  title: string
  summary: string
  updatedAt?: number
  // Só planos: o estado do plano e a coluna do card ligado.
  state?: BacklogState
  column?: BacklogColumn
}

export interface ArtifactGroup<T extends BrowsableArtifact = BrowsableArtifact> {
  id: ArtifactGroupId
  entries: T[]
}

export const ALL_STATES = 'todos' satisfies StateFilter
export const GROUP_ORDER: readonly ArtifactGroupId[] = ['precisa', 'execucao', 'prontos', 'concluidos', 'documentos']

const STATE_GROUP: Record<Exclude<StateFilter, 'todos'>, ArtifactGroupId> = {
  proposta: 'precisa',
  execucao: 'execucao',
  pronto: 'prontos',
  concluido: 'concluidos',
}
const IN_EXECUTION_COLUMNS: readonly BacklogColumn[] = ['In progress', 'In review']
const DIACRITICS = /\p{Diacritic}/gu
const NO_DATE = 0

function normalized(text: string): string {
  return text.normalize('NFD').replace(DIACRITICS, '').toLocaleLowerCase('pt-BR')
}

export function matchesQuery(entry: BrowsableArtifact, query: string): boolean {
  const needle = normalized(query.trim())
  if (needle === '') return true
  return [entry.title, entry.id, plainSummary(entry.summary)].some((field) => normalized(field).includes(needle))
}

// Mesmas entradas do selo do card (planChip), para o grupo e o selo nunca discordarem.
export function groupOf(entry: BrowsableArtifact): ArtifactGroupId {
  if (entry.state === undefined) return 'documentos'
  if (entry.state === 'proposto') return 'precisa'
  if (entry.state !== 'vinculado') return 'execucao'
  if (entry.column === 'Done') return 'concluidos'
  return entry.column !== undefined && IN_EXECUTION_COLUMNS.includes(entry.column) ? 'execucao' : 'prontos'
}

export function matchesState(entry: BrowsableArtifact, filter: StateFilter): boolean {
  return filter === ALL_STATES || groupOf(entry) === STATE_GROUP[filter]
}

export function groupEntries<T extends BrowsableArtifact>(entries: readonly T[]): ArtifactGroup<T>[] {
  return GROUP_ORDER.map((id) => ({
    id,
    entries: entries
      .filter((entry) => groupOf(entry) === id)
      .sort((a, b) => (b.updatedAt ?? NO_DATE) - (a.updatedAt ?? NO_DATE) || a.id.localeCompare(b.id)),
  })).filter((group) => group.entries.length > 0)
}
