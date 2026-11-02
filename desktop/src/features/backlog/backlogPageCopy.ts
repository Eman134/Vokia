import type { BacklogColumn } from './backlogTypes'

export const BACKLOG_TITLE = 'Backlog e planos'
export const BACKLOG_SUBTITLE =
  'Todo pedido entra aqui. Features viram um card pai: a Iara faz a triagem e desdobra em micro-tarefas, o time executa, você revisa e elas são entregues.'
export const NEW_REQUEST_LABEL = '+ Novo pedido'
export const PLAN_ALL_LABEL = 'Triar os {count} pendentes'
export const ALL_TRIAGED_LABEL = 'Tudo triado'
export const SEARCH_LABEL = 'Buscar no backlog'
export const SEARCH_PLACEHOLDER = 'Buscar item'
export const MINE_LABEL = 'Meus pedidos'
export const EMPTY_COLUMN_TEXT = 'Nada aqui'

export const DENSITY_OPTIONS = [
  { label: 'Normal', value: 'normal' },
  { label: 'Compacta', value: 'compacta' },
] as const
export const TYPE_FILTER_OPTIONS = [
  { label: 'Tudo', value: 'tudo' },
  { label: 'Features', value: 'feature' },
  { label: 'Micro-tarefas', value: 'micro' },
] as const

export const ALL_FEATURES_VALUE = 'todas'
export const ALL_FEATURES_LABEL = 'Todas'
export const LOADING_TEXT = 'Carregando o backlog…'
export const EMPTY_BOARD_TITLE = 'Nenhum item neste projeto.'
export const EMPTY_BOARD_TEXT = 'Quando um pedido virar item, ele aparece aqui.'
export const LOAD_ERROR_PREFIX = 'Não foi possível carregar o backlog: '

export interface ColumnMeta {
  name: BacklogColumn
  hint: string
  dotToken: string
}

export const COLUMN_META: readonly ColumnMeta[] = [
  { name: 'Backlog', hint: 'pedidos e ideias; passam pela triagem aqui', dotToken: 'faint' },
  { name: 'Ready', hint: 'plano aprovado, esperando o time', dotToken: 'accent' },
  { name: 'In progress', hint: 'um agente está trabalhando', dotToken: 'accent' },
  { name: 'In review', hint: 'diff esperando revisão', dotToken: 'warn' },
  { name: 'Done', hint: 'entregue nesta semana', dotToken: 'ok' },
]
