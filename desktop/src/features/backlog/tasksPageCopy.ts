import type { BacklogColumn } from './backlogTypes'

export const TASKS_TITLE = 'Tasks'
export const SPRINTS_BUTTON_LABEL = 'Sprints'
export const SPRINT_ALL_LABEL = 'Sprint: todas'
export const SPRINT_NONE_LABEL = 'Sem sprint'
export const NEW_REQUEST_LABEL = 'Novo pedido'
export const NEW_REQUEST_DIALOG_TITLE = 'Novo pedido'
export const NEW_REQUEST_TITLE_LABEL = 'Título'
export const NEW_REQUEST_DESCRIPTION_LABEL = 'Descrição'
export const NEW_REQUEST_TITLE_PLACEHOLDER = 'O que você quer que o time faça?'
export const NEW_REQUEST_DESCRIPTION_PLACEHOLDER = 'Contexto, critérios, links (opcional)'
export const NEW_REQUEST_CANCEL_LABEL = 'Cancelar'
export const NEW_REQUEST_SUBMIT_LABEL = 'Criar pedido'
export const NEW_REQUEST_CREATING_LABEL = 'Criando…'
export const NEW_REQUEST_NO_PROJECT_NOTICE = 'Selecione um projeto para criar um pedido.'
export const EMPTY_COLUMN_TEXT = 'nada por aqui'

export const ALL_FEATURES_VALUE = 'todas'
export const ALL_FEATURES_LABEL = 'Todas'
export const LOADING_TEXT = 'Carregando o backlog…'
export const EMPTY_BOARD_TITLE = 'Nenhum item neste projeto.'
export const EMPTY_BOARD_TEXT = 'Quando um pedido virar item, ele aparece aqui.'
export const LOAD_ERROR_PREFIX = 'Não foi possível carregar o backlog: '

export interface ColumnMeta {
  name: BacklogColumn
  colorToken: string
}

export const COLUMN_META: readonly ColumnMeta[] = [
  { name: 'Backlog', colorToken: 'faint' },
  { name: 'Ready', colorToken: 'accent' },
  { name: 'In progress', colorToken: 'accent' },
  { name: 'In review', colorToken: 'warn' },
  { name: 'Done', colorToken: 'ok' },
]
