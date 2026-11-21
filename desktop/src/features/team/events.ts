export const VOKIA_EVENT = 'vokia://event'

export type Agent = string

export type Speaker = 'user' | Agent

export type AgentState = 'sending' | 'thinking' | 'waiting' | 'idle'

export type ToolCategory = 'read' | 'edit' | 'command' | 'search' | 'list' | 'web' | 'mcp' | 'app' | 'agent' | 'plan' | 'other'

export type ToolStatus = 'running' | 'ok' | 'failed'

export type CommandDomain =
  | 'team'
  | 'projects'
  | 'channels'
  | 'backlog'
  | 'artifacts'
  | 'attachments'
  | 'agents'
  | 'roles'
  | 'orchestration'
  | 'providers'
  | 'settings'

export type ToolDetail =
  | { kind: 'read'; from: number; to: number }
  | { kind: 'found'; count: number; names: string[] }
  | { kind: 'created'; id?: string }
  | { kind: 'updated'; taskId: string; status: string }
  | { kind: 'started'; cardId: string }
  | { kind: 'command'; command: string; domain?: CommandDomain; subject?: string }

export interface ToolChange {
  added: string
  removed?: string
}

export interface ToolActivity {
  callId: string
  name: string
  category: ToolCategory
  target: string
  status: ToolStatus
  added?: number
  removed?: number
  change?: ToolChange
  durationMs?: number
  detail?: ToolDetail
  result?: string
}

export interface TurnActivity {
  thinkingMs?: number
  thinkingText?: string
  tools: ToolActivity[]
}

export interface RequestProposal {
  title: string
  detail: string
}

export interface TurnUsage {
  provider?: string
  model?: string
  inputTokens?: number
  outputTokens?: number
  cachedInputTokens?: number
  costUsd?: number
}

export interface VokiaMessage {
  type: 'message'
  from: Speaker
  to: Speaker
  text: string
  time: number
  projectId: number | null
  channelId?: number
  activity?: TurnActivity
  requestProposal?: RequestProposal
  usage?: TurnUsage
  isError?: boolean
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

export type PermissionResultOutcome = PermissionOutcome | 'project-rule'

export interface FeedPermissionResult {
  type: 'permission-result'
  agent: Agent
  detail: string
  outcome: PermissionResultOutcome
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
  suggestAlways: boolean
}

export interface VokiaPermissionClosed {
  type: 'permission-closed'
  id: string
  projectId: number | null
  outcome: PermissionOutcome
}

export interface VokiaPermissionByRule {
  type: 'permission-by-rule'
  agent: Agent
  detail: string
  projectId: number | null
}

export type DecisionKind = 'aprovacao' | 'escolha' | 'pergunta' | 'revisao' | 'validacao'

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
  outcome: 'finished' | 'failed' | 'paused'
}

export type UsageLimitWindow = 'five-hour' | 'seven-day'

export interface VokiaUsageLimit {
  type: 'usage-limit'
  agent: Agent
  projectId: number
  window: UsageLimitWindow
  resetsAt: number
  runId: number | null
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

export interface VokiaTaskResult {
  type: 'task-result'
  projectId: number
  objective: string
  concluded: boolean
  turns: number
  agents: Agent[]
  time: number
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

export interface VokiaBacklogChanged {
  type: 'backlog-changed'
  projectId: number
}

export interface VokiaArtifactsChanged {
  type: 'artifacts-changed'
  projectId: number
}

export interface VokiaOrchestrationChanged {
  type: 'orchestration-changed'
}

export interface VokiaBatchProgress {
  type: 'batch-progress'
  projectId: number
  leader: Agent
  returned: number
  total: number
  running: Agent[]
  done: boolean
}

export interface VokiaAgentNamed {
  type: 'agent-named'
  projectId: number
  agent: Agent
  name: string
}

export interface VokiaAgentActivity {
  type: 'agent-activity'
  agent: Agent
  projectId: number
  activity: TurnActivity
}

export type OverrideReason = 'usage-limit' | 'model-unavailable' | 'manual-temp'

export interface TemporaryOverride {
  agent: Agent
  cli: string
  model: string
  modelTier?: string | null
  command: string
  originalCli: string
  originalModel: string
  originalTier?: string | null
  reason: OverrideReason
  appliedAt: number
  expiresAt?: number | null
}

export interface VokiaTemporaryOverridesChanged {
  type: 'temporary-overrides-changed'
  overrides: TemporaryOverride[]
}

export interface VokiaModelUnavailable {
  type: 'model-unavailable'
  agent: Agent
  provider: string
  model: string
  detail: string
}

export type VokiaEvent =
  | VokiaMessage
  | VokiaAgentState
  | VokiaAgentActivity
  | VokiaError
  | VokiaPermission
  | VokiaPermissionClosed
  | VokiaPermissionByRule
  | VokiaDecision
  | VokiaPlanUsage
  | VokiaUsageLimit
  | VokiaRequestSettled
  | VokiaConsoleLine
  | VokiaTaskResult
  | VokiaGoalState
  | VokiaBacklogChanged
  | VokiaArtifactsChanged
  | VokiaOrchestrationChanged
  | VokiaAgentNamed
  | VokiaBatchProgress
  | VokiaTemporaryOverridesChanged
  | VokiaModelUnavailable

export interface CliProblem {
  reason: string
  fixCommand: string
}
