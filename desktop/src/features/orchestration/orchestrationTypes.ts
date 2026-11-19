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

export const ORCH_ACTION_VALUES = ['delegar', 'devolver', 'escalar'] as const
export type OrchAction = (typeof ORCH_ACTION_VALUES)[number]

export const EXEC_MODE_VALUES = ['sequencia', 'paralelo'] as const
export type ExecMode = (typeof EXEC_MODE_VALUES)[number]

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
}

export interface OrchPlacement {
  agentId: Agent
  teamId: string
  layerId: string
  instructions: string
  maxInstances: number
}

export interface OrchRule {
  id: number
  fromLayer: string
  toLayer: string
  whenEvent: WhenEvent
  action: OrchAction
  kind: RequestKind
  execMode: ExecMode
  returnLimit: number
  position: number
}

export interface OrchestrationSnapshot {
  settings: OrchestrationSettings
  layers: OrchLayer[]
  teams: OrchTeam[]
  placements: OrchPlacement[]
  rules: OrchRule[]
}

export interface OrchTeamDraft {
  id: string | null
  name: string
}

export interface OrchPlacementDraft {
  agentId: Agent
  teamId: string
  layerId: string
  instructions: string
  maxInstances: number
}
