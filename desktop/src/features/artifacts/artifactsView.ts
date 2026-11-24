import { ALL_STATES, type StateFilter } from './artifactBrowse'

export type ArtifactsLayout = 'grade' | 'lista'

export interface ArtifactsView {
  query: string
  state: StateFilter
  layout: ArtifactsLayout
  doneOpen: boolean
}

export const DEFAULT_VIEW: ArtifactsView = { query: '', state: ALL_STATES, layout: 'grade', doneOpen: false }

const STORAGE_PREFIX = 'vokia.artefatos.'
const STATES: readonly StateFilter[] = ['todos', 'proposta', 'pronto', 'execucao', 'concluido']
const LAYOUTS: readonly ArtifactsLayout[] = ['grade', 'lista']

function storageKey(projectId: number): string {
  return `${STORAGE_PREFIX}${projectId}`
}

function parse(raw: unknown): ArtifactsView {
  const value = typeof raw === 'object' && raw !== null ? (raw as Partial<Record<keyof ArtifactsView, unknown>>) : {}
  return {
    query: typeof value.query === 'string' ? value.query : DEFAULT_VIEW.query,
    state: STATES.find((state) => state === value.state) ?? DEFAULT_VIEW.state,
    layout: LAYOUTS.find((layout) => layout === value.layout) ?? DEFAULT_VIEW.layout,
    doneOpen: typeof value.doneOpen === 'boolean' ? value.doneOpen : DEFAULT_VIEW.doneOpen,
  }
}

export function readView(projectId: number): ArtifactsView {
  try {
    const raw = localStorage.getItem(storageKey(projectId))
    return raw === null ? { ...DEFAULT_VIEW } : parse(JSON.parse(raw))
  } catch {
    // Registro ilegível ou armazenamento indisponível: volta ao padrão.
    return { ...DEFAULT_VIEW }
  }
}

export function writeView(projectId: number, view: ArtifactsView): void {
  try {
    localStorage.setItem(storageKey(projectId), JSON.stringify(view))
  } catch {
    // Sem armazenamento, a escolha vale só nesta sessão.
  }
}
