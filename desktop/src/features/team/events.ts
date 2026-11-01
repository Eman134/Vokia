export const VOKIA_EVENT = 'vokia://event'

export type Agent = 'iara' | 'davi'

export type Speaker = 'user' | Agent

export type AgentState = 'thinking' | 'waiting' | 'idle'

export interface VokiaMessage {
  type: 'message'
  from: Speaker
  to: Speaker
  text: string
  time: number
  projectId: number | null
}

export interface VokiaAgentState {
  type: 'agent-state'
  agent: Agent
  state: AgentState
  projectId: number | null
}

export interface FeedNotice {
  type: 'notice'
  text: string
  time: number
}

export interface VokiaError {
  type: 'error'
  message: string
}

export interface VokiaPermission {
  type: 'permission'
  id: string
  agent: Agent
  tool: string
  detail: string
  projectId: number | null
}

export interface VokiaPermissionClosed {
  type: 'permission-closed'
  id: string
}

export type DecisionKind = 'aprovacao' | 'escolha' | 'pergunta' | 'revisao'

export interface VokiaDecision {
  type: 'decision'
  id: number
  agent: Agent
  kind: DecisionKind
  title: string
  detail: string
  options: string[]
  projectId: number | null
}

export interface UsageWindow {
  percent: number
  resetsAt: number
}

export interface VokiaPlanUsage {
  type: 'plan-usage'
  fiveHour: UsageWindow
  sevenDay: UsageWindow
}

export type VokiaEvent =
  | VokiaMessage
  | VokiaAgentState
  | VokiaError
  | VokiaPermission
  | VokiaPermissionClosed
  | VokiaDecision
  | VokiaPlanUsage

export interface CliProblem {
  reason: string
  fixCommand: string
}
