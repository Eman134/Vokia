import type { BacklogColumn } from '../backlog/backlogTypes'

export const MAP_TITLE = 'Mapa do time'
export const MAP_SUBTITLE =
  'Uma linha por time, uma coluna por camada. A tarefa anda para a direita e volta quando a revisão reprova.'
export const TAB_TEAMS = 'Times e camadas'
export const TAB_ORCHESTRATION = 'Orquestração'
export const ADD_TEAM_LABEL = '+ Time'
export const ADD_LAYER_LABEL = '+ Camada'
export const EMPTY_CELL_LABEL = 'Posicionar agente existente'
export const AGENT_PICK_LABEL = 'Agente'
export const CORNER_LABEL = 'time ↓ · camada →'
export const UNPLACED_TITLE = 'Sem posição'
export const PLACE_LABEL = 'Posicionar'
export const NO_LAYERS_TITLE = 'Nenhuma camada ainda.'
export const CREATE_LAYER_LABEL = 'Criar camada'
export const NO_TEAMS_TITLE = 'Nenhum time ainda.'
export const CREATE_TEAM_LABEL = 'Criar time'
export const LOAD_ERROR_TITLE = 'Não foi possível carregar o mapa.'
export const NO_PROJECT_TITLE = 'Selecione um projeto para ver o mapa.'
export const MOVEMENTS_TITLE = 'Movimentos'
export const NO_MOVEMENTS_TITLE = 'Nenhum movimento ainda.'
export const FLOW_TITLE = 'Como o fluxo anda'
export const EDIT_IN_ORCHESTRATION_LABEL = 'Editar na orquestração'
export const NO_RULES_OFF_TEXT = 'Nenhuma seta ainda: qualquer agente pode passar trabalho a qualquer outro.'
export const NO_RULES_ON_TEXT = 'Nenhuma seta ainda: com as regras ligadas, toda delegação é recusada.'
export const NEW_TEAM_TITLE = 'Novo time'
export const NEW_LAYER_TITLE = 'Nova camada'
export const PLACE_AGENT_TITLE = 'Posicionar agente'
export const DESCRIPTION_LABEL = 'Descrição'
export const TYPE_LABEL = 'Tipo'
export const PLACE_PLACEHOLDER = 'Escolha…'
export const IN_PROGRESS_COLUMN: BacklogColumn = 'In progress'

export function agentCountLabel(count: number): string {
  if (count === 0) return 'sem agentes'
  return count === 1 ? '1 agente' : `${count} agentes`
}

export function teamInitial(name: string): string {
  return name.trim().charAt(0).toUpperCase()
}
