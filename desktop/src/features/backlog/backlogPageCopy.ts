import type { BacklogColumn } from './backlogTypes'

export const BACKLOG_TITLE = 'Tasks'
export const BACKLOG_SUBTITLE =
  'Todo pedido entra aqui. Features viram um card pai: Ember faz a triagem e desdobra em micro-tarefas, o time executa, você revisa e elas são entregues.'
export const NEW_REQUEST_LABEL = '+ Novo pedido'
export const NEW_REQUEST_DIALOG_TITLE = 'Novo pedido'
export const NEW_REQUEST_TITLE_LABEL = 'Título'
export const NEW_REQUEST_DESCRIPTION_LABEL = 'Descrição'
export const NEW_REQUEST_TITLE_PLACEHOLDER = 'O que você quer que o time faça?'
export const NEW_REQUEST_DESCRIPTION_PLACEHOLDER = 'Contexto, critérios, links (opcional)'
export const NEW_REQUEST_CANCEL_LABEL = 'Cancelar'
export const NEW_REQUEST_SUBMIT_LABEL = 'Criar pedido'
export const NEW_REQUEST_CREATING_LABEL = 'Criando…'
export const NEW_REQUEST_NO_PROJECT_NOTICE = 'Selecione um projeto para criar um pedido.'
export const PLAN_ALL_LABEL = 'Triar os {count} pendentes'
export const ALL_TRIAGED_LABEL = 'Tudo triado'
export const SEARCH_LABEL = 'Buscar no backlog'
export const SEARCH_PLACEHOLDER = 'Buscar item'
export const MINE_LABEL = 'Meus pedidos'
export const EMPTY_COLUMN_TEXT = 'Vazio'

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
  dotToken: string
}

export const COLUMN_META: readonly ColumnMeta[] = [
  { name: 'Backlog', dotToken: 'faint' },
  { name: 'Ready', dotToken: 'accent' },
  { name: 'In progress', dotToken: 'accent' },
  { name: 'In review', dotToken: 'warn' },
  { name: 'Done', dotToken: 'ok' },
]
