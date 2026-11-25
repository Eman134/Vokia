import type { OrchTaskStage } from '../orchestration/orchestrationTypes'
import type { ExecutionMode } from '../permissions/executionModes'

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
  | 'usage'
  | 'analytics'
  | 'search'

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
  description?: string
  added?: number
  removed?: number
  change?: ToolChange
  durationMs?: number
  detail?: ToolDetail
  result?: string
}

export type TurnPlanStepStatus = 'pending' | 'in_progress' | 'completed'

export interface TurnPlanStep {
  id: string
  text: string
  activeText?: string
  status: TurnPlanStepStatus
}

export interface TurnPlan {
  explanation?: string
  steps: TurnPlanStep[]
}

export interface TurnPlanSnapshot {
  projectId: number
  agent: Agent
  cardId?: string
  turnId: string
  plan: TurnPlan
  updatedAt: number
  active: boolean
}

export interface TurnActivity {
  thinkingMs?: number
  thinkingText?: string
  tools: ToolActivity[]
  currentStepLabel?: string
  plan?: TurnPlan
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
  cacheWriteTokens?: number
  thoughtTokens?: number
  costUsd?: number
  isFallback?: boolean
  contextTokens?: number
  contextWindow?: number
}

export interface VokiaMessage {
  type: 'message'
  from: Speaker
  to: Speaker
  text: string
  time: number
  projectId: number | null
  channelId?: number
  itemId?: string
  activity?: TurnActivity
  requestProposal?: RequestProposal
  usage?: TurnUsage
  isError?: boolean
  /** true quando algum lado veio de uma instância excluída; `from`/`to` trazem o id bruto, que pode não estar em `Speaker`. */
  foreignInstance?: boolean
}

export interface VokiaAgentState {
  type: 'agent-state'
  agent: Agent
  state: AgentState
  projectId: number | null
  itemId?: string
}

export interface FeedNotice {
  type: 'notice'
  text: string
  time: number
}

export type PermissionOutcome = 'allowed' | 'denied' | 'expired'

export type PermissionResultOutcome = PermissionOutcome | 'project-rule' | AutoApprovalMode

/** Modos que aprovam pedidos sozinhos, sem regra salva. */
export type AutoApprovalMode = Extract<ExecutionMode, 'auto' | 'bypass'>

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
  rootCommand?: string | null
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
  /** Presente quando a aprovação veio do modo de execução do projeto, e não de uma regra salva. */
  mode?: AutoApprovalMode
}

export interface VokiaExecutionModeChanged {
  type: 'execution-mode-changed'
  projectId: number
  mode: ExecutionMode
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

export type UsageWindowKind =
  | 'five-hour'
  | 'daily'
  | 'weekly'
  | 'monthly'
  | { custom: number }

export type PaceStatus = 'sustainable' | 'above-pace'

export interface ProviderUsageWindow {
  percent: number
  resetsAt: number
  durationMins: number
  kind: UsageWindowKind
  label?: string
  rejected?: boolean
}

export interface ProviderUsageGroup {
  id: string
  label: string
  models?: string[]
  window: ProviderUsageWindow
}

export interface ProviderPlanUsage {
  providerId: string
  label?: string
  primary?: ProviderUsageWindow
  secondary?: ProviderUsageWindow
  tertiary?: ProviderUsageWindow
  groups?: ProviderUsageGroup[]
  creditsAvailable?: boolean
  resetCreditsAvailable?: number
  updatedAtMs: number
  readFailed: boolean
}

export interface UsageLimitEntry {
  providerId: string
  model: string
  resetsAt?: number
  releaseAtMs: number
  markedAtMs: number
}

export interface UsageLimitReturn {
  agent: Agent
  originalProviderId: string
  originalModel: string
  originalTier?: string | null
  fallbackProviderId: string
  fallbackModel: string
  releaseAtMs: number
}

export interface PlanUsageSnapshot {
  providers: ProviderPlanUsage[]
  activeLimits: UsageLimitEntry[]
  pendingReturns: UsageLimitReturn[]
  warnThreshold: number
  pauseThreshold: number
  fetchedAtMs: number
}

export interface VokiaProviderUsageUpdated {
  type: 'provider-usage-updated'
  usage: ProviderPlanUsage
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

export interface VokiaUsageLimitFailed {
  type: 'usage-limit-failed'
  agent: Agent
  projectId: number
  window?: UsageLimitWindow
  resetsAt?: number
  message: string
  channelId?: number
  itemId?: string
}

export interface FeedUsageLimitFailed extends VokiaUsageLimitFailed {
  time: number
}

export interface AgentPause {
  projectId: number
  agent: Agent
}

export interface VokiaAgentPausesChanged {
  type: 'agent-pauses-changed'
  pauses: AgentPause[]
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
  channelId?: number | null
  itemId?: string | null
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

export interface VokiaTaskStage {
  type: 'task-stage'
  stage: OrchTaskStage
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
  itemId?: string
}

export interface VokiaTurnPlanUpdated {
  type: 'turn-plan-updated'
  projectId: number
  agent: Agent
  cardId?: string
  turnId: string
  plan: TurnPlan
}

export type ContinuationTrigger = 'plan' | 'next_step' | 'finish_check' | 'compaction' | 'provider_exit'

export interface VokiaTurnContinuation {
  type: 'turn-continuation'
  projectId: number
  agent: Agent
  trigger: ContinuationTrigger
  attempt: number
  maxAttempts: number
  marker: string
  time: number
  channelId?: number
  itemId?: string
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
  | VokiaTurnPlanUpdated
  | VokiaTurnContinuation
  | VokiaError
  | VokiaPermission
  | VokiaPermissionClosed
  | VokiaPermissionByRule
  | VokiaExecutionModeChanged
  | VokiaDecision
  | VokiaPlanUsage
  | VokiaProviderUsageUpdated
  | VokiaUsageLimit
  | VokiaUsageLimitFailed
  | VokiaAgentPausesChanged
  | VokiaRequestSettled
  | VokiaConsoleLine
  | VokiaTaskResult
  | VokiaGoalState
  | VokiaBacklogChanged
  | VokiaArtifactsChanged
  | VokiaOrchestrationChanged
  | VokiaAgentNamed
  | VokiaBatchProgress
  | VokiaTaskStage
  | VokiaTemporaryOverridesChanged
  | VokiaModelUnavailable

export interface CliProblem {
  reason: string
  fixCommand: string
}
