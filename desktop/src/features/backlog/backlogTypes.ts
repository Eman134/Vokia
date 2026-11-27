export type BacklogState = 'vazio' | 'triagem' | 'gerando' | 'proposto' | 'vinculado'

export type KanbanStage = 'ready' | 'progress' | 'review' | 'done'

export type BacklogTag = string


export type TaskCardAction = 'triar' | 'revisar' | 'vincular' | 'iniciar'

export type BacklogOrigin = 'usuario' | 'conversa' | 'agente'

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

// Rótulo na tela; o valor interno (API, MCP, vokia-cli) continua o de BacklogColumn.
export const COLUMN_LABEL: Record<BacklogColumn, string> = {
  Backlog: 'A fazer',
  Ready: 'Pronto',
  'In progress': 'Em andamento',
  'In review': 'Em revisão',
  Done: 'Concluído',
}

export const MOVABLE_COLUMNS: readonly BacklogColumn[] = ['Backlog', 'Ready', 'In progress', 'In review', 'Done']
export const DEFAULT_COLUMN: BacklogColumn = 'Backlog'
// O dono "voce" é o padrão de card criado pelo usuário; o backend só troca o responsável quando ele é esse.
export const USER_OWNER = 'voce'
export const UNASSIGNED_TEXT = 'Sem responsável'
export const isUnassigned = (item: BacklogItem): boolean => item.who === USER_OWNER || item.who === ''
export const AUTO_COLUMNS_HINT = `${COLUMN_LABEL['In progress']} e ${COLUMN_LABEL['In review']} andam sozinhos`
export const READY_COLUMN: BacklogColumn = 'Ready'
export const READY_BLOCKED_HINT = `${COLUMN_LABEL.Ready} é para item com plano revisado; sem plano, inicie a task direto`
export const DELEGATE_LABEL = 'Delegar a'
export const UNLINKABLE_COLUMNS: readonly BacklogColumn[] = ['Backlog', 'Ready']
export const OTHER_OPTION = 'Outro…'
export const QUESTIONS_TITLE = (leader: string): string => `Perguntas de ${leader} · triagem`
export const OTHER_PLACEHOLDER = (leader: string): string => `Escreva sua resposta para a ${leader}`
export const OTHER_INPUT_LABEL = 'Sua resposta'
export const MICRO_SUMMARY = (total: number, done: number): string => `${total} micro-tarefas · ${done} feitas`
export const MICRO_EXPLANATION = (leader: string): string =>
  `Cada micro-tarefa vem com um roteiro fechado: contexto, arquivos, passos, teste e limites. Modelos leves só executam; quem decide é a ${leader}. Se uma micro-tarefa volta da revisão, a ${leader} detalha o roteiro com o que faltou e o mesmo modelo tenta de novo; só na 2ª vez sobe o modelo; depois disso, chama você. O que faltou vira fato na memória, e os próximos roteiros parecidos já nascem detalhados.`
export const CONVERSATIONS_LABEL = 'Abrir conversas'
export const DECISION_QUESTION_PREFIX = 'dec-'
export const MAIN_ACTION_LABEL = 'Iniciar'
export const MODAL_STATE_CHIP: Record<BacklogState, string> = {
  triagem: 'Triagem',
  vazio: 'Não triado',
  gerando: 'Escrevendo plano',
  proposto: 'Proposta',
  vinculado: COLUMN_LABEL.Ready,
}
export const PLAN_HINT: Record<BacklogState, string> = {
  triagem: 'A líder lê o código e a memória, pergunta o que faltar e desdobra a feature.',
  vazio: 'O plano é opcional: inicie a task direto, mande para a triagem da líder ou vincule um plano que já existe.',
  gerando: 'Atualiza sozinho enquanto o agente escreve.',
  proposto: 'Aprove ou ajuste a proposta; o plano é opcional e você pode iniciar a task sem ele.',
  vinculado: 'Os cards entram no kanban do time.',
}

export interface BacklogItem {
  id: string
  publicId?: string
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
  planNote?: string
  children?: { total: number; done: number }
  who: 'voce' | string
  by: string
  origin: BacklogOrigin
  branch: string | null
  worktreePath: string | null
  // Quem concluiu o card (a validação automática ou o usuário) e quantas devoluções automáticas ele já teve.
  closedBy?: 'usuario' | 'automatico' | null
  autoAttempts?: number
  isNew?: boolean
  column?: BacklogColumn
  priority?: string
  estimate?: string
  questions?: TriageQuestion[]
  criteria?: BacklogCriterion[]
  activity?: BacklogActivity[]
  links?: BacklogLink[]
  error?: BacklogItemError
}

export type RequestOrigin = Exclude<BacklogOrigin, 'agente'>

export const ORIGIN_USER: RequestOrigin = 'usuario'

export interface NewRequestDraft {
  title: string
  description: string
  origin: RequestOrigin
  links: BacklogLink[]
}

export interface BacklogItemError {
  reason: string
}
