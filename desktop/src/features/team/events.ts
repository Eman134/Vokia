export const VOKIA_EVENT = 'vokia://event'

export type Agent = string

export type Speaker = 'user' | Agent

export type AgentState = 'sending' | 'thinking' | 'waiting' | 'idle'

export interface VokiaMessage {
  type: 'message'
  from: Speaker
  to: Speaker
  text: string
  time: number
  projectId: number | null
  channelId?: number
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

export type PermissionOutcome = 'allowed' | 'denied' | 'expired'

export interface FeedPermissionResult {
  type: 'permission-result'
  agent: Agent
  detail: string
  outcome: PermissionOutcome
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
  expiresAt: number
  approvedBefore: number
}

export interface VokiaPermissionClosed {
  type: 'permission-closed'
  id: string
  projectId: number | null
  outcome: PermissionOutcome
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
  itemId: string | null
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

export interface VokiaRequestSettled {
  type: 'request-settled'
  runId: number
  projectId: number
  outcome: 'finished' | 'failed'
}

export type ConsoleKind = 'start' | 'text' | 'tool' | 'output' | 'error' | 'permission' | 'done' | 'failed'

export interface ConsoleEntry {
  seq: number
  kind: ConsoleKind
  text: string
  time: number
}

export interface VokiaConsoleLine extends ConsoleEntry {
  type: 'console-line'
  agent: Agent
  projectId: number
}

export type GoalState = 'atingido' | 'incompleto' | 'sem-avanco' | 'repetindo'

export interface VokiaGoalState {
  type: 'goal-state'
  projectId: number
  state: GoalState | null
  objective: string
  reason: string
  checks: number
}

export type VokiaEvent =
  | VokiaMessage
  | VokiaAgentState
  | VokiaError
  | VokiaPermission
  | VokiaPermissionClosed
  | VokiaDecision
  | VokiaPlanUsage
  | VokiaRequestSettled
  | VokiaConsoleLine
  | VokiaGoalState

export interface CliProblem {
  reason: string
  fixCommand: string
}
