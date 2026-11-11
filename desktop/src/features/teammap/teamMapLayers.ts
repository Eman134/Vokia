export type MapLayerId = 'lideranca' | 'implementacao' | 'revisao'

export const MAP_LAYERS: readonly { id: MapLayerId; title: string; description: string }[] = [
  { id: 'lideranca', title: 'Liderança', description: 'planeja e delega' },
  { id: 'implementacao', title: 'Implementação', description: 'escreve o código' },
  { id: 'revisao', title: 'Revisão', description: 'testa e aprova' },
]

const DEFAULT_LAYER: MapLayerId = 'implementacao'

// Camada ainda não é dado do agente; Map evita que ids como 'constructor' resolvam para o prototype.
const LAYER_BY_AGENT: ReadonlyMap<string, MapLayerId> = new Map<string, MapLayerId>([
  ['iara', 'lideranca'],
  ['davi', 'implementacao'],
  ['nina', 'implementacao'],
  ['rui', 'implementacao'],
  ['teo', 'revisao'],
])

export function layerOf(agentId: string): MapLayerId {
  return LAYER_BY_AGENT.get(agentId) ?? DEFAULT_LAYER
}

export const MAP_TITLE = 'Mapa do time'
export const MAP_SUBTITLE =
  'Uma linha por time, uma coluna por camada. A tarefa anda para a direita e volta quando a revisão reprova.'
export const TAB_TEAMS = 'Times e camadas'
export const TAB_ORCHESTRATION = 'Orquestração'
export const ADD_TEAM_LABEL = '+ Time'
export const ADD_LAYER_LABEL = '+ Camada'
export const ADD_AGENT_LABEL = '+ Agente'
export const EMPTY_CELL_LABEL = '+ agente'
export const CORNER_LABEL = 'time ↓ · camada →'
export const TEAM_ROW_NAME = 'Time'
export const TEAM_ROW_INITIAL = 'T'

export function agentCountLabel(count: number): string {
  if (count === 0) return 'sem agentes'
  return count === 1 ? '1 agente' : `${count} agentes`
}
