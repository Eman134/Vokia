export type BacklogState = 'vazio' | 'triagem' | 'gerando' | 'proposto' | 'vinculado'

export type KanbanStage = 'ready' | 'progress' | 'review' | 'done'

export type BacklogTag = 'bug' | 'urgente' | 'ui' | 'backend' | 'ideia' | 'divida' | 'cliente'

export type BacklogCardDensity = 'normal' | 'compacta'

export type BacklogCardAction = 'triar' | 'revisar' | 'ver-diff'

export type BacklogColumn = 'Backlog' | 'Ready' | 'In progress' | 'In review' | 'Done'

export interface TriageQuestion {
  id: string
  text: string
  options: string[]
  selected?: string
}

export interface ScriptBlock {
  context: string
  files: string[]
  steps: string[]
  done: string[]
  limits: string[]
}

export type MicroTaskStatus = 'proposta' | 'ready' | 'em andamento' | 'em revisão' | 'feito'

export interface MicroTask {
  id: string
  title: string
  who: string
  model: string
  status: MicroTaskStatus
  script: ScriptBlock
  returned?: string[]
  learned?: string[]
}

export interface BacklogCriterion {
  id: string
  text: string
  done: boolean
}

export interface BacklogActivity {
  id: string
  text: string
  at: string
}

export interface BacklogLink {
  id: string
  label: string
}

export const MOVABLE_COLUMNS: readonly BacklogColumn[] = ['Backlog', 'Ready', 'Done']
export const DEFAULT_COLUMN: BacklogColumn = 'Backlog'
export const AUTO_COLUMNS_HINT = 'In progress e In review andam sozinhos'
export const OTHER_OPTION = 'Outro…'
export const MAIN_ACTION: Record<BacklogState, string> = {
  triagem: 'Responder à líder',
  vazio: 'Mandar para a triagem',
  gerando: 'Ver o plano em andamento',
  proposto: 'Ver a proposta',
  vinculado: 'Rodar',
}
export const MODAL_STATE_CHIP: Record<BacklogState, string> = {
  triagem: 'Triagem',
  vazio: 'Não triado',
  gerando: 'Escrevendo plano',
  proposto: 'Proposta',
  vinculado: 'Pronto para o time',
}
export const PLAN_HINT: Record<BacklogState, string> = {
  triagem: 'A líder lê o código e a memória, pergunta o que faltar e desdobra a feature.',
  vazio: 'Mande para a triagem da líder ou vincule um plano que já existe.',
  gerando: 'Atualiza sozinho enquanto o agente escreve.',
  proposto: 'Aprove ou ajuste antes de o time começar.',
  vinculado: 'Os cards entram no kanban do time.',
}

export interface BacklogItem {
  id: string
  title: string
  type: 'task' | 'feature'
  state: BacklogState
  stage?: KanbanStage
  triageStep?: 1 | 2 | 3
  parent?: string
  feature?: string
  tags: BacklogTag[]
  description?: string
  plan?: string
  children?: { total: number; done: number }
  who: 'voce' | string
  by: string
  isNew?: boolean
  column?: BacklogColumn
  priority?: string
  estimate?: string
  questions?: TriageQuestion[]
  micro?: MicroTask[]
  criteria?: BacklogCriterion[]
  activity?: BacklogActivity[]
  links?: BacklogLink[]
  error?: BacklogItemError
}

export interface BacklogItemError {
  reason: string
}
