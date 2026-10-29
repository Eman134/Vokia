export type BacklogState = 'vazio' | 'triagem' | 'gerando' | 'proposto' | 'vinculado'

export type KanbanStage = 'ready' | 'progress' | 'review' | 'done'

export type BacklogTag = 'bug' | 'urgente' | 'ui' | 'backend' | 'ideia' | 'divida' | 'cliente'

export type BacklogCardDensity = 'normal' | 'compacta'

export type BacklogCardAction = 'triar' | 'revisar' | 'ver-diff'

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
}
