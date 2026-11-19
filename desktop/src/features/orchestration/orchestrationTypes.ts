import type { Agent } from '../team/events'

export const ENFORCEMENT_VALUES = ['off', 'on'] as const
export type Enforcement = (typeof ENFORCEMENT_VALUES)[number]

export const STEP_MODE_VALUES = ['lotes', 'livre'] as const
export type StepMode = (typeof STEP_MODE_VALUES)[number]

export const HUMAN_GATE_VALUES = ['lote', 'fim', 'nunca'] as const
export type HumanGate = (typeof HUMAN_GATE_VALUES)[number]

export const WORKTREE_POLICY_VALUES = ['sempre', 'paralelo', 'nunca'] as const
export type WorktreePolicy = (typeof WORKTREE_POLICY_VALUES)[number]

export const LAYER_KIND_VALUES = ['planejar', 'implementar', 'revisar', 'pesquisar', 'livre'] as const
export type LayerKind = (typeof LAYER_KIND_VALUES)[number]

export const REQUEST_KIND_VALUES = ['planejar', 'implementar', 'testar', 'revisar', 'pesquisar'] as const
export type RequestKind = (typeof REQUEST_KIND_VALUES)[number]

export const WHEN_EVENT_VALUES = [
  'plano_aprovado',
  'tarefa_concluida',
  'revisao_reprovou',
  'testes_falharam',
  'revisao_aprovou',
  'pedido_de_ajuda',
] as const
export type WhenEvent = (typeof WHEN_EVENT_VALUES)[number]

export const FLOW_START = '__inicio'
export const FLOW_END = '__fim'

export const FINAL_VALIDATION_VALUES = ['manual', 'automatica'] as const
export type FinalValidation = (typeof FINAL_VALIDATION_VALUES)[number]

export const GATEWAY_KIND_VALUES = ['paralelo', 'exclusivo'] as const
export type GatewayKind = (typeof GATEWAY_KIND_VALUES)[number]

export const LINK_VALUES = ['fluxo', 'comunicacao'] as const
export type FlowLink = (typeof LINK_VALUES)[number]

export const SYNC_MODE_VALUES = ['sincrona', 'assincrona'] as const
export type SyncMode = (typeof SYNC_MODE_VALUES)[number]

export const EDGE_ACTION_VALUES = ['delegar', 'devolver', 'escalar', 'consultar'] as const
export type EdgeAction = (typeof EDGE_ACTION_VALUES)[number]

export interface OrchestrationSettings {
  enforcement: Enforcement
  leaderAgent: Agent
  stepMode: StepMode
  humanGate: HumanGate
  maxParallel: number
  pairTurnLimit: number
  taskTurnLimit: number
  worktreePolicy: WorktreePolicy
}

export interface OrchLayer {
  id: string
  name: string
  description: string
  position: number
  kind: LayerKind
  instructions: string
}

export interface OrchTeam {
  id: string
  name: string
  position: number
  leaderAgent: Agent | null
  instructions: string
  finalValidation: FinalValidation
  startX: number | null
  startY: number | null
  endX: number | null
  endY: number | null
}

export interface OrchPlacement {
  agentId: Agent
  teamId: string
  layerId: string
  instructions: string
  maxInstances: number
  posX: number
  posY: number
  criteria: string
  maxWorktrees: number | null
}

export interface OrchFlowEdge {
  id: number
  teamId: string
  fromNode: string
  toNode: string
  link: FlowLink
  syncMode: SyncMode
  whenEvent: WhenEvent
  isDefault: boolean
  action: EdgeAction
  kind: RequestKind
  turnLimit: number | null
  curve: number
  instructions: string
  position: number
}

export interface OrchFlowGateway {
  id: string
  teamId: string
  kind: GatewayKind
  posX: number
  posY: number
}

export interface OrchestrationSnapshot {
  settings: OrchestrationSettings
  layers: OrchLayer[]
  teams: OrchTeam[]
  placements: OrchPlacement[]
  edges: OrchFlowEdge[]
  gateways: OrchFlowGateway[]
}

export interface OrchTeamDraft {
  id: string | null
  name: string
  instructions: string
}

export interface OrchPlacementDraft {
  agentId: Agent
  teamId: string
  layerId: string
  instructions: string
  maxInstances: number
  // Ausentes, o backend mantém a posição, os critérios e o limite de worktrees que o agente já tem.
  posX?: number | null
  posY?: number | null
  criteria?: string
  maxWorktrees?: number | null
}

export interface FlowNodeDraft {
  agentId: Agent
  layerId: string
  posX: number
  posY: number
}

export interface FlowGatewayDraft {
  id: string
  kind: GatewayKind
  posX: number
  posY: number
}

export interface FlowEdgeDraft {
  fromNode: string
  toNode: string
  link: FlowLink
  syncMode: SyncMode
  whenEvent: WhenEvent
  isDefault: boolean
  action: EdgeAction
  kind: RequestKind
  turnLimit: number | null
  curve: number
  instructions: string
}

export interface FlowPoint {
  x: number
  y: number
}

export interface TeamFlowDraft {
  nodes: FlowNodeDraft[]
  gateways: FlowGatewayDraft[]
  edges: FlowEdgeDraft[]
  start: FlowPoint | null
  end: FlowPoint | null
}

export const FLOW_PROBLEM_CODES = ['V1', 'V2', 'V3', 'V4', 'V5', 'V6', 'V7'] as const
export type FlowProblemCode = (typeof FLOW_PROBLEM_CODES)[number]

// O save_team_flow recusado devolve a lista destes problemas como texto JSON.
export interface FlowProblem {
  code: FlowProblemCode
  node: string | null
  message: string
}
